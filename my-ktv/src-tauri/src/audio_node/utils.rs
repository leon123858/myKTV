use cpal::traits::DeviceTrait;
use cpal::{SampleFormat, StreamConfig, SupportedStreamConfigRange};

pub fn generate_output_resolve_config(
    device_name: String
) -> impl FnMut(&cpal::Device) -> Result<StreamConfig, String> {
    move |device: &cpal::Device| -> Result<StreamConfig, String> {
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

                        break 'search;
                    }
                }
            }
        }

        let picked_config = picked_config.expect("[HAL] Failed to find any compatible config!");

        Ok(picked_config)
    }
}

pub fn generate_input_resolve_config(
    device_name: String
) -> impl FnMut(&cpal::Device) -> Result<StreamConfig, String> {
    move |device: &cpal::Device| -> Result<StreamConfig, String> {
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

                        break 'search;
                    }
                }
            }
        }

        let picked_config = picked_config.expect("[HAL] Failed to find any compatible config!");

        Ok(picked_config)
    }
}