use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{ConvolverConfig, ConvolverNode, MicrophoneNode, NodeType};
use std::sync::atomic::Ordering;
use std::time::Duration;

fn main() {
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    let dest_id = ctx.build_graph(|builder| {
        println!("Creating microphone node");
        let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to access microphone");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        let ir_path = "examples/resource/hall01.wav";
        let convolver_node = ConvolverNode::from_file_with_config(
            ir_path,
            sample_rate,
            None,
            ConvolverConfig {
                stereo: true,
                growth_exponent: 2,
                ..Default::default()
            },
        )
        .expect("Unable to construct ConvolverNode");
        let drop_count = convolver_node.clone_drop_count();

        std::thread::spawn(move || {
            let mut last_drop = 0;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let current_drop = drop_count.load(Ordering::Relaxed);

                if current_drop > last_drop {
                    println!(
                        "⚠️ Reverb processing too slow! Total drops: {} (New: {})",
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
                    "⚠️ Audio thread too slow! Late callbacks count: {} (New {}), Current CPU load: {}%",
                    current_late_callbacks,
                    current_late_callbacks - last_late_callbacks,
                    load_percent
                );
                last_late_callbacks = current_late_callbacks;
            } else if load_percent > 80 {
                // optional warning if approaching critical load
                println!(
                    "⚠️ Audio thread load too high! Current CPU load: {}%",
                    load_percent
                );
            }
        }
    });

    println!("========================================");
    println!("🎤 Capturing microphone with synthetic Reverb effect...");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
