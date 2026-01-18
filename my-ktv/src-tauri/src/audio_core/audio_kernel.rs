use crate::audio_core::const_number::{
    PREFERRED_BUFFER_SIZE, PREFERRED_SAMPLE_RATE, RING_BUFFER_CAPACITY,
};
use crate::audio_core::AudioKernel;
use crate::dsp::{AudioProcessor, GainProcessor};
use anyhow::{Context, Result};
use audio_thread_priority::promote_current_thread_to_real_time;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::RingBuffer;

impl AudioKernel {
    pub fn init() -> Result<Self> {
        let host = cpal::default_host();

        println!("[HAL] Audio Host: {:?}", host.id());

        // 獲取輸出設備 (DAC)
        let output_device = host
            .default_output_device()
            .context("No output device available")?;
        println!("[HAL] Output Device: {}", output_device.name()?);

        // 獲取輸入設備 (ADC / Mic)
        let input_device = host.default_input_device();
        if let Some(ref d) = input_device {
            println!("[HAL] Input Device: {}", d.name()?);
        }

        // 協商並建立輸出流
        let output_config = resolve_config(&output_device)?;
        println!("[HAL] Negotiated Output Config: {:?}", output_config);

        // 建立 Lock-free Ring Buffer
        let (producer, mut consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);

        // 初始化 DSP 鏈
        let mut gain_effect = GainProcessor::new(-6.0); // 預設 -6dB
        gain_effect.prepare(output_config.sample_rate.0 as f32);

        // 建立輸出 Callback (ISR)
        let err_fn = |err| eprintln!("[HAL] Output Stream Error: {}", err);

        let output_stream = output_device.build_output_stream(
            &output_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // --- 實時音訊線程 (Real-time Audio Thread) ---

                // 確保線程優先級 (僅在第一次運行時生效)
                ensure_realtime_priority();

                // read ring buffer data and send to output stream data
                let read_chunk_result = consumer.read_chunk(data.len());
                match read_chunk_result {
                    Ok(chunk) => {
                        // read chunk may be two split slice in ring buffer
                        // because ring buffer's ring back property
                        let (first, second) = chunk.as_slices();

                        let first_len = first.len();
                        let second_len = second.len();
                        let mut data_idx = 0;
                        for i in 0..first_len {
                            data[data_idx] = first[i];
                            data_idx += 1
                        }
                        for i in 0..second_len {
                            data[data_idx] = second[i];
                            data_idx += 1
                        }
                        assert_eq!(data_idx, data.len());

                        // move ring buf ptr
                        chunk.commit_all();
                    }
                    Err(_) => {
                        // println!("chunk read error: {:?}", err);
                        data.fill(0.0);
                        return;
                    }
                }

                // handle DSP
                for frame in data.chunks_mut(2) {
                    if frame.len() != 2 {
                        continue;
                    }
                    if let [left, right] = frame {
                        gain_effect.process(left, right);
                    }
                }

                // (Optional) 產生一個簡單的正弦波測試硬體是否工作
                // generate_sine_wave(data);
            },
            err_fn,
            None, // Timeout: blocking negotiation
        )?;

        // 4. 啟動引擎
        output_stream.play()?;

        Ok(Self {
            _output_stream: output_stream,
            _input_stream: None,
            audio_producer: producer,
        })
    }
}

// --- 輔助函數：硬體配置協商 ---

fn resolve_config(device: &cpal::Device) -> Result<cpal::StreamConfig> {
    let supported_configs = device.supported_output_configs()?;

    // 策略 A: 尋找原生支援 48kHz + f32 格式的配置
    // 避免 Windows Audio Engine 進行隱式重採樣 (SRC) 導致音質下降和延遲
    let chosen_range = supported_configs
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
        .find(|c| {
            c.max_sample_rate().0 >= PREFERRED_SAMPLE_RATE
                && c.min_sample_rate().0 <= PREFERRED_SAMPLE_RATE
        })
        .or_else(|| device.supported_output_configs().ok()?.next()) // Fallback
        .context("Failed to find a supported config")?;

    // 策略 B: 鎖定緩衝區大小 (Latency Tuning)
    // 我們嘗試請求硬體允許的最小值，但不能低於我們的目標值以免爆音
    let buffer_size = match chosen_range.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            let target = PREFERRED_BUFFER_SIZE.clamp(*min, *max);
            println!(
                "[HAL] Buffer Negotiation: Requesting {}, Range [{}-{}]",
                target, min, max
            );
            cpal::BufferSize::Fixed(target)
        }
        cpal::SupportedBufferSize::Unknown => cpal::BufferSize::Default,
    };

    Ok(cpal::StreamConfig {
        channels: 2, // 強制立體聲
        sample_rate: cpal::SampleRate(PREFERRED_SAMPLE_RATE),
        buffer_size,
    })
}

// --- 線程優先級管理 (類似 RTOS Task Priority) ---

// 使用 std::sync::Once 確保只執行一次
static PRIORITY_SET: std::sync::Once = std::sync::Once::new();

fn ensure_realtime_priority() {
    PRIORITY_SET.call_once(|| {
        // 請求 512 frames 的計算預算，48kHz
        match promote_current_thread_to_real_time(512, 48000) {
            Ok(_) => println!(" Audio thread priority boosted!"),
            Err(e) => eprintln!(" Failed to boost thread priority: {}", e),
        }
    });
}

// 測試用：產生 440Hz 正弦波
// static mut PHASE: f32 = 0.0;
// fn generate_sine_wave(data: &mut [f32]) {
//     unsafe {
//         for chunk in data.chunks_mut(2) {
//             let val = (PHASE * 2.0 * std::f32::consts::PI).sin() * 0.1; // 0.1 gain
//             if chunk.len() >= 2 {
//                 chunk[0] = val; // 左聲道
//                 chunk[1] = val; // 右聲道
//             }
//             PHASE = (PHASE + 440.0 / 48000.0) % 1.0;
//         }
//     }
// }
