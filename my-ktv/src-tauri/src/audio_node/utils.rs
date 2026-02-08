use cpal::traits::DeviceTrait;
use cpal::{FromSample, Sample, SampleFormat, StreamConfig};
use rtrb::{Consumer, Producer};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

#[derive(Debug, Clone)]
pub struct IOStreamConfig {
    pub sample_format: SampleFormat,
    pub stream_config: StreamConfig,
}

pub fn generate_output_resolve_config(
    device_name: String,
) -> impl FnMut(&cpal::Device) -> Result<IOStreamConfig, String> {
    move |device: &cpal::Device| -> Result<IOStreamConfig, String> {
        let supported_configs: Vec<_> = device
            .supported_output_configs()
            .expect("no supported config")
            .collect();

        println!("[HAL] Supported Configs: {:?}", supported_configs);

        let priority_channels = [Some(2), Some(1), None];
        let priority_formats = [
            SampleFormat::F32,
            SampleFormat::I32,
            SampleFormat::I16,
            SampleFormat::U8,
        ];
        let priority_rates = [48000, 44100, 9600];

        let mut picked_config: Option<StreamConfig> = None;
        let mut picked_format: Option<SampleFormat> = None;

        'search: for target_channel in priority_channels.iter() {
            for format in priority_formats.iter() {
                for rate in priority_rates.iter() {
                    let matching_range = supported_configs.iter().find(|conf| {
                        let format_match = conf.sample_format() == *format;
                        let rate_match =
                            *rate >= conf.min_sample_rate() && *rate <= conf.max_sample_rate();
                        let channel_match = match target_channel {
                            Some(ch) => conf.channels() == *ch,
                            None => true,
                        };

                        format_match && rate_match && channel_match
                    });

                    if let Some(range) = matching_range {
                        println!("[HAL] {:?} Match Found!", device_name);
                        println!(
                            "      Channel: {:?} (Requested: {:?})",
                            range.channels(),
                            target_channel
                        );
                        println!("      Format : {:?}", format);
                        println!("      Rate   : {:?}", rate);

                        picked_config = Some(range.with_sample_rate(*rate).into());
                        picked_format = Some(*format);

                        break 'search;
                    }
                }
            }
        }

        let picked_config = picked_config.expect("[HAL] Failed to find any compatible config!");
        let picked_format = picked_format.expect("[HAL] Failed to find any compatible format!");

        Ok(IOStreamConfig {
            sample_format: picked_format,
            stream_config: picked_config,
        })
    }
}

pub fn generate_input_resolve_config(
    device_name: String,
) -> impl FnMut(&cpal::Device) -> Result<IOStreamConfig, String> {
    move |device: &cpal::Device| -> Result<IOStreamConfig, String> {
        let supported_configs: Vec<_> = device
            .supported_input_configs()
            .expect("no supported config")
            .collect();

        println!("[HAL] Supported Configs: {:?}", supported_configs);

        let priority_channels = [Some(2), Some(1), None];
        let priority_formats = [
            SampleFormat::F32,
            SampleFormat::I32,
            SampleFormat::I16,
            SampleFormat::U8,
        ];
        let priority_rates = [48000, 44100, 9600];

        let mut picked_config: Option<StreamConfig> = None;
        let mut picked_format: Option<SampleFormat> = None;

        'search: for target_channel in priority_channels.iter() {
            for format in priority_formats.iter() {
                for rate in priority_rates.iter() {
                    let matching_range = supported_configs.iter().find(|conf| {
                        let format_match = conf.sample_format() == *format;
                        let rate_match =
                            *rate >= conf.min_sample_rate() && *rate <= conf.max_sample_rate();
                        let channel_match = match target_channel {
                            Some(ch) => conf.channels() == *ch,
                            None => true,
                        };

                        format_match && rate_match && channel_match
                    });

                    if let Some(range) = matching_range {
                        println!("[HAL] {:?} Match Found!", device_name);
                        println!(
                            "      Channel: {:?} (Requested: {:?})",
                            range.channels(),
                            target_channel
                        );
                        println!("      Format : {:?}", format);
                        println!("      Rate   : {:?}", rate);

                        picked_config = Some(range.with_sample_rate(*rate).into());
                        picked_format = Some(*format);

                        break 'search;
                    }
                }
            }
        }

        let picked_config = picked_config.expect("[HAL] Failed to find any compatible config!");
        let picked_format = picked_format.expect("[HAL] Failed to find any compatible format!");

        Ok(IOStreamConfig {
            sample_format: picked_format,
            stream_config: picked_config,
        })
    }
}

pub struct ResamplingHandler {
    resampler: SincFixedIn<f32>,
    input_channels: Vec<Vec<f32>>,
    output_channels: Vec<Vec<f32>>,
    src_channels_cnt: usize,
    target_channels_cnt: usize,
    pub producer: Producer<f32>,
    inner_producer: Producer<f32>,
    inner_consumer: Consumer<f32>,
}

impl ResamplingHandler {
    pub fn new(
        producer: Producer<f32>,
        src_cfg: StreamConfig,
        target_cfg: StreamConfig,
        inner_producer: Producer<f32>,
        inner_consumer: Consumer<f32>,
        max_frames: usize,
    ) -> Self {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        if inner_producer.slots() < max_frames {
            panic!("mid buf should bigger than max frame");
        }

        let src_channels = src_cfg.channels as usize;
        let target_channels = target_cfg.channels as usize;

        let src_sample_rate = src_cfg.sample_rate;
        let target_sample_rate = target_cfg.sample_rate;

        // 預估輸出緩衝區大小（加上安全邊際）
        let ratio = (target_sample_rate / src_sample_rate) as f64;

        let resampler =
            SincFixedIn::<f32>::new(ratio, 2.0, params, max_frames, src_channels).unwrap();
        let input_channels = resampler.input_buffer_allocate(true);
        let output_channels = resampler.output_buffer_allocate(true);

        Self {
            resampler,
            input_channels,
            output_channels,
            src_channels_cnt: src_channels,
            target_channels_cnt: target_channels,
            producer,
            inner_producer,
            inner_consumer,
        }
    }

    pub fn process_packet<T>(&mut self, input_data: &[T])
    where
        T: Sample,
        f32: FromSample<T>,
    {
        // push data into resample buffer
        let should_push_data_cnt = input_data.len().min(self.inner_producer.slots());
        if should_push_data_cnt == 0 {
            println!("[HAL] should_push_data_cnt is zero");
            return;
        }
        match self.inner_producer.write_chunk(should_push_data_cnt) {
            Ok(mut chunk) => {
                let (first, second) = chunk.as_mut_slices();
                let mid = first.len();
                let mut cursor = 0;
                for i in 0..mid {
                    first[cursor] = input_data[i].to_sample::<f32>();
                    cursor += 1;
                }
                cursor = 0;
                for i in mid..should_push_data_cnt {
                    second[cursor] = input_data[i].to_sample::<f32>();
                    cursor += 1;
                }
                chunk.commit_all();
            }
            Err(err) => {
                println!("[FileSrc] Failed to push sample: {}", err);
                return;
            }
        };

        loop {
            let one_round_needed_sample_cnt = Resampler::input_frames_max(&self.resampler)
                * Resampler::nbr_channels(&self.resampler);

            // check enough to resample
            if self.inner_consumer.slots() < one_round_needed_sample_cnt {
                // println!("[HAL] not enough samples to resample");
                return;
            }

            // Deinterleave
            match self.inner_consumer.read_chunk(one_round_needed_sample_cnt) {
                Ok(chunk) => {
                    let (first, second) = chunk.as_slices();
                    let mut cursor = 0;
                    for a in [first, second].iter() {
                        for v in a.iter() {
                            let chan = cursor % self.src_channels_cnt;
                            let idx = cursor / self.src_channels_cnt;
                            self.input_channels[chan][idx] = *v;
                            cursor += 1;
                        }
                    }
                    chunk.commit_all();
                }
                Err(err) => {
                    println!("[HAL] Error reading data {:?}", err);
                    return;
                }
            }

            // resample
            let ret = Resampler::process_into_buffer(
                &mut self.resampler,
                &mut self.input_channels,
                &mut self.output_channels,
                None,
            );
            match ret {
                Ok((_read, written)) => {
                    self.handle_output(written);
                }
                Err(error) => {
                    println!("[HAL] Error processing data: {}", error);
                }
            }
        }
    }

    pub fn check_must_loss_all_data(&mut self) -> bool {
        let size = self.producer.slots();
        let needed = self.target_channels_cnt;
        needed >= size
    }

    pub fn check_must_no_loss_data(&mut self, to_write: usize) -> bool {
        let can_write_frame = to_write + (self.inner_consumer.slots() / self.src_channels_cnt);
        let can_write_frame_with_chan = can_write_frame * self.target_channels_cnt;
        can_write_frame_with_chan < self.producer.slots()
    }

    fn handle_output(&mut self, written: usize) {
        // multi-chan algo: only dup first channel
        let mut not_sent = written;
        // println!("[HAL] not_sent {} samples to producer", not_sent);
        let first_channel = &self.output_channels[0];
        for idx in 0..first_channel.len() {
            if self.producer.slots() < self.target_channels_cnt {
                println!("[HAL] resample Output buffer full");
                break;
            }
            if not_sent == 0 {
                break;
            }
            for _ in 0..self.target_channels_cnt {
                if self.producer.push(first_channel[idx]).is_err() {
                    println!("[HAL] Error sending resample data to producer {}", idx);
                    break;
                }
            }
            not_sent -= 1;
        }
    }
}
