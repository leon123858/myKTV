#[cfg(test)]
mod tests {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, SampleRate, SupportedStreamConfigRange};
    use rtrb::RingBuffer; // 改用 rtrb
    use std::time::Duration;

    // 輔助函數：找出兩者都支援的「採樣率」，但不強求聲道數相同
    fn find_common_sample_rate(
        input: &cpal::Device,
        output: &cpal::Device,
    ) -> anyhow::Result<SampleRate> {
        let input_configs: Vec<SupportedStreamConfigRange> = input
            .supported_input_configs()?
            .filter(|c| c.sample_format() == SampleFormat::F32)
            .collect();

        let output_configs: Vec<SupportedStreamConfigRange> = output
            .supported_output_configs()?
            .filter(|c| c.sample_format() == SampleFormat::F32)
            .collect();

        // 常見的標準採樣率優先嘗試
        let standard_rates = [48000, 44100, 96000, 88200];

        for &rate in &standard_rates {
            let r = rate;
            let in_support = input_configs
                .iter()
                .any(|c| c.min_sample_rate() <= r && c.max_sample_rate() >= r);
            let out_support = output_configs
                .iter()
                .any(|c| c.min_sample_rate() <= r && c.max_sample_rate() >= r);

            if in_support && out_support {
                return Ok(r);
            }
        }

        anyhow::bail!("No common standard sample rate found (tried 48k, 44.1k, etc).")
    }

    #[test]
    #[ignore]
    fn test_audio_feedback_resilient() -> anyhow::Result<()> {
        let latency_ms = 500.0;
        let test_duration = Duration::from_secs(20);

        let host = cpal::default_host();
        let input_device = host.default_input_device().expect("no input device");
        let output_device = host.default_output_device().expect("no output device");

        println!("Input: {}", input_device.description()?.name());
        println!("Output: {}", output_device.description()?.name());

        // 1. 協商採樣率 (Sample Rate)
        let common_sample_rate = find_common_sample_rate(&input_device, &output_device)?;
        println!("Negotiated Sample Rate: {}", common_sample_rate);

        // 2. 建立各自的 Config (允許聲道數不同)
        let mut in_config_range = input_device
            .supported_input_configs()?
            .find(|c| {
                c.min_sample_rate() <= common_sample_rate
                    && c.max_sample_rate() >= common_sample_rate
                    && c.sample_format() == SampleFormat::F32
            })
            .expect("should have valid input config")
            .with_sample_rate(common_sample_rate);

        let mut out_config_range = output_device
            .supported_output_configs()?
            .find(|c| {
                c.min_sample_rate() <= common_sample_rate
                    && c.max_sample_rate() >= common_sample_rate
                    && c.sample_format() == SampleFormat::F32
            })
            .expect("should have valid output config")
            .with_sample_rate(common_sample_rate);

        // 強制轉為標準 StreamConfig
        let input_config: cpal::StreamConfig = in_config_range.into();
        let output_config: cpal::StreamConfig = out_config_range.into();

        let input_channels = input_config.channels as usize;
        let output_channels = output_config.channels as usize;

        println!(
            "Config: Input {}ch -> Output {}ch",
            input_channels, output_channels
        );

        // 3. RingBuffer 計算 (改用 rtrb)
        let latency_frames = (latency_ms / 1_000.0) * common_sample_rate as f32;
        let latency_samples = latency_frames as usize * input_channels;

        // rtrb::RingBuffer::new 直接回傳 (Producer, Consumer)
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(latency_samples * 4);

        // 填入靜音
        for _ in 0..latency_samples {
            let _ = producer.push(0.0);
        }

        // 4. Callback 定義
        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            for &sample in data {
                let _ = producer.push(sample);
            }
        };

        // Output Callback 負責 "聲道轉換"
        let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // 用來替代 smallvec 的固定大小緩衝區 (Stack allocation)
            // 32 聲道對於一般應用已經非常足夠
            const MAX_CHANNELS: usize = 32;
            let mut input_frame_buf = [0.0f32; MAX_CHANNELS];

            for output_frame in data.chunks_mut(output_channels) {
                // 1. 從 RingBuffer 讀取一個 Input Frame 的數據
                for i in 0..input_channels {
                    let sample = consumer.pop().unwrap_or(0.0);
                    if i < MAX_CHANNELS {
                        input_frame_buf[i] = sample;
                    }
                    // 如果 input_channels 超過 MAX_CHANNELS (極少見)，多餘的就讀出丟棄以保持同步
                }

                // 2. 進行聲道混合 (Mixing / Remapping)
                // 注意：這裡使用 input_frame_buf 而不是 smallvec
                if input_channels == 1 && output_channels == 2 {
                    // Mono -> Stereo
                    let s = input_frame_buf[0];
                    output_frame[0] = s;
                    output_frame[1] = s;
                } else if input_channels == 2 && output_channels == 1 {
                    // Stereo -> Mono
                    let s = (input_frame_buf[0] + input_frame_buf[1]) / 2.0;
                    output_frame[0] = s;
                } else if input_channels == output_channels {
                    // 1:1 複製
                    for (i, s) in output_frame.iter_mut().enumerate() {
                        if i < MAX_CHANNELS {
                            *s = input_frame_buf[i];
                        } else {
                            *s = 0.0;
                        }
                    }
                } else {
                    // 其他情況：只複製能對應的部分
                    for (i, out_sample) in output_frame.iter_mut().enumerate() {
                        if i < input_channels && i < MAX_CHANNELS {
                            *out_sample = input_frame_buf[i];
                        } else {
                            *out_sample = 0.0;
                        }
                    }
                }
            }
        };

        let err_fn = |err| eprintln!("Stream error: {}", err);

        let input_stream =
            input_device.build_input_stream(&input_config, input_data_fn, err_fn, None)?;
        let output_stream =
            output_device.build_output_stream(&output_config, output_data_fn, err_fn, None)?;

        input_stream.play()?;
        output_stream.play()?;

        println!("Playing for {:?}...", test_duration);
        std::thread::sleep(test_duration);

        Ok(())
    }
}
