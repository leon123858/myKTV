use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType, RING_BUFFER_CAPACITY};
use audio_thread_priority::promote_current_thread_to_real_time;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamError};
use rtrb::{Consumer, Producer, RingBuffer};

// 系統採樣率標準：KTV 系統強烈建議鎖定 48kHz
pub const PREFERRED_SAMPLE_RATE: u32 = 48000;
// 延遲目標：128 samples @ 48kHz ~= 2.6ms (單向)
pub const PREFERRED_BUFFER_SIZE: u32 = 128;

pub struct SpeakerDest {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    pub output_stream: Stream,
}

impl AudioNode for SpeakerDest {
    fn init() -> Self {
        let host = cpal::default_host();
        println!("[HAL] Audio Host: {:?}", host.id());

        // 獲取輸出設備 (DAC)
        let output_device = host
            .default_output_device()
            .expect("no output device available");
        println!("[HAL] Output Device: {:?}", output_device.name());

        // 協商並建立輸出流
        let output_config =
            resolve_config(&output_device).expect("failed to resolve output device config");
        println!("[HAL] Negotiated Output Config: {:?}", output_config);

        // 建立 Lock-free Ring Buffer
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);

        let output_stream_ret = output_device.build_output_stream(
            &output_config,
            data_hdl_cb_creator(consumer),
            err_hdl_cb,
            None, // Timeout: blocking negotiation
        );

        let output_stream = output_stream_ret.expect("output stream created error");

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: Option::from(producer),
            output_stream,
        }
    }

    fn start(&mut self) {
        self.output_stream.play().expect("failed to start stream");
        self.state = AudioNodeState::RUNNING
    }

    fn stop(&mut self) {
        self.output_stream.pause().expect("failed to start stream");
        self.state = AudioNodeState::STOPPED
    }

    fn get_type(&self) -> AudioNodeType {
        AudioNodeType::DESTINATION
    }

    fn get_state(&self) -> AudioNodeState {
        self.state.clone()
    }
}

fn err_hdl_cb(err: StreamError) {
    eprintln!("[HAL] Output Stream Error: {}", err);
}

fn data_hdl_cb_creator(
    mut consumer: Consumer<f32>,
) -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) {
    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
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
    }
}

fn resolve_config(device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    let supported_configs = device
        .supported_output_configs()
        .expect("no supported config");

    // 策略 A: 尋找原生支援 48kHz + f32 格式的配置
    // 避免 Windows Audio Engine 進行隱式重採樣 (SRC) 導致音質下降和延遲
    let chosen_range = supported_configs
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
        .find(|c| {
            c.max_sample_rate().0 >= PREFERRED_SAMPLE_RATE
                && c.min_sample_rate().0 <= PREFERRED_SAMPLE_RATE
        })
        .or_else(|| device.supported_output_configs().ok()?.next())
        .expect("no supported chosen_range");

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
