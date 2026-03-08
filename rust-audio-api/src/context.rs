use crate::node::AudioNode;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use std::sync::{Arc, Mutex};

pub struct AudioContext {
    stream: Option<Stream>,
    sample_rate: u32,
    // 簡單起見，我們先用 Mutex 包裝一個 Node 作為輸出來源
    // 在真正的即時音訊處理中，通常會使用 lock-free 資料結構來取代 Mutex
    destination_node: Arc<Mutex<Option<Box<dyn AudioNode>>>>,
}

impl AudioContext {
    pub fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("找不到音訊輸出設備");
        let config = device.default_output_config()?;

        Ok(Self {
            stream: None,
            sample_rate: config.sample_rate(),
            destination_node: Arc::new(Mutex::new(None)),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 將一個 Node 連接到最終輸出
    pub fn connect_destination(&mut self, node: Box<dyn AudioNode>) {
        let mut dest = self.destination_node.lock().unwrap();
        *dest = Some(node);
    }

    /// 開始播放音訊
    pub fn resume(&mut self) -> Result<(), anyhow::Error> {
        if self.stream.is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let config: StreamConfig = device.default_output_config()?.into();

        let channels = config.channels as usize;
        let dest_node_clone = Arc::clone(&self.destination_node);

        // 建立 cpal 的資料流
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut node_lock = dest_node_clone.lock().unwrap();

                // 走訪每一個音訊 frame
                for frame in data.chunks_mut(channels) {
                    let output = if let Some(ref mut node) = *node_lock {
                        node.process()
                    } else {
                        [0.0, 0.0] // 如果沒有連接節點，輸出靜音
                    };

                    // 將算出的左右聲道寫入 cpal buffer
                    if channels >= 2 {
                        frame[0] = output[0];
                        frame[1] = output[1];
                    } else if channels == 1 {
                        frame[0] = (output[0] + output[1]) / 2.0; // Stereo downmix to Mono
                    }
                }
            },
            |err| eprintln!("音訊串流發生錯誤: {}", err),
            None,
        )?;

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }
}
