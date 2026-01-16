use anyhow::{Context, Result};
use audio_thread_priority::{promote_current_thread_to_real_time, RtPriorityHandle};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct SendWrapper<T>(pub T);

unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

// 系統採樣率標準：KTV 系統強烈建議鎖定 48kHz
const PREFERRED_SAMPLE_RATE: u32 = 48000;
// 延遲目標：128 samples @ 48kHz ~= 2.6ms (單向)
const PREFERRED_BUFFER_SIZE: u32 = 128;

pub struct AudioKernel {
    _output_stream: SendWrapper<cpal::Stream>,
    _input_stream: Option<SendWrapper<cpal::Stream>>,
}

impl AudioKernel {
    pub fn init() -> Result<Self> {
        let host = cpal::default_host();

        println!("[HAL] Audio Host: {:?}", host.id());

        // 1. 獲取輸出設備 (DAC)
        let output_device = host
            .default_output_device()
            .context("No output device available")?;
        println!("[HAL] Output Device: {}", output_device.name()?);

        // 2. 獲取輸入設備 (ADC / Mic)
        // 注意：在 WASAPI Shared Mode 下，輸入和輸出是分開的設備
        let input_device = host.default_input_device();
        if let Some(ref d) = input_device {
            println!("[HAL] Input Device: {}", d.name()?);
        }

        // 3. 協商並建立輸出流
        let output_config = resolve_config(&output_device)?;
        println!("[HAL] Negotiated Output Config: {:?}", output_config);

        // 建立輸出 Callback (ISR)
        let err_fn = |err| eprintln!("[HAL] Output Stream Error: {}", err);

        let output_stream = output_device.build_output_stream(
            &output_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // --- 實時音訊線程 (Real-time Audio Thread) ---
                // 這裡相當於嵌入式的中斷處理函數 (ISR)

                // 1. 確保線程優先級 (僅在第一次運行時生效)
                ensure_realtime_priority();

                // 2. 填充靜音 (防止爆音)，Phase 2 將在此處接入 RingBuffer
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }

                // 3. (Optional) 產生一個簡單的正弦波測試硬體是否工作
                generate_sine_wave(data);
            },
            err_fn,
            None, // Timeout: blocking negotiation
        )?;

        // 4. 啟動引擎
        output_stream.play()?;

        Ok(Self {
            _output_stream: SendWrapper(output_stream),
            _input_stream: None,
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
static mut PHASE: f32 = 0.0;
fn generate_sine_wave(data: &mut [f32]) {
    unsafe {
        for chunk in data.chunks_mut(2) {
            let val = (PHASE * 2.0 * std::f32::consts::PI).sin() * 0.1; // 0.1 gain
            if chunk.len() >= 2 {
                chunk[0] = val; // 左聲道
                chunk[1] = val; // 右聲道
            }
            PHASE = (PHASE + 440.0 / 48000.0) % 1.0;
        }
    }
}
