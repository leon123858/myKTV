use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{ConvolverConfig, ConvolverNode, MicrophoneNode, NodeType};
use std::sync::atomic::Ordering;
use std::time::Duration;

fn main() {
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    let dest_id = ctx.build_graph(|builder| {
        println!("建立麥克風節點");
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法存取麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        let ir_path = "examples/resource/hall01.wav";
        let convolver_node = ConvolverNode::from_file_with_config(
            ir_path,
            sample_rate,
            None,
            ConvolverConfig {
                stereo: true,
                growth_exponent: 2,
            },
        )
        .expect("無法建構 ConvolverNode");
        let drop_count = convolver_node.clone_drop_count();

        std::thread::spawn(move || {
            let mut last_drop = 0;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let current_drop = drop_count.load(Ordering::Relaxed);

                if current_drop > last_drop {
                    println!(
                        "⚠️ Reverb 處理來不及！總掉幀次數: {} (新增: {})",
                        current_drop,
                        current_drop - last_drop
                    );
                    last_drop = current_drop;
                }
            }
        });

        let convolver = builder.add_node(NodeType::Convolver(convolver_node));

        builder.connect(mic, convolver);

        convolver
    });

    ctx.resume(dest_id).unwrap();

    let monitor = ctx.performance_monitor();
    std::thread::spawn(move || {
        let mut last_late_callbacks = 0;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let current_late_callbacks = monitor.late_callbacks.load(Ordering::Relaxed);
            let load_percent = monitor.current_load_percent.load(Ordering::Relaxed);

            if current_late_callbacks > last_late_callbacks {
                println!(
                    "⚠️ 音訊主執行緒處理太慢！延遲發生次數: {} (新增 {}), 當前 CPU 負載: {}%",
                    current_late_callbacks,
                    current_late_callbacks - last_late_callbacks,
                    load_percent
                );
                last_late_callbacks = current_late_callbacks;
            } else if load_percent > 80 {
                // optional warning if approaching critical load
                println!("⚠️ 音訊主執行緒負載過高！當前 CPU 負載: {}%", load_percent);
            }
        }
    });

    println!("========================================");
    println!("🎤 正在播放帶有合成 Reverb 特效的麥克風聲音中...");
    println!("⌨️  按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
