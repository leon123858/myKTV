use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{ConvolverNode, MicrophoneNode, NodeType};
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

        println!("合成 IR: 1.5 秒殘響");
        let ir_len = (sample_rate as f32 * 1.5) as usize;
        let mut ir = vec![[0.0; 2]; ir_len];
        
        // for i in 0..ir_len {
        //     let t = i as f32 / sample_rate as f32;
        //     let decay = (-5.0 * t).exp();
        //     let noise = (rand::random::<f32>() * 2.0 - 1.0) * decay;
        //     ir[i][0] = noise;
        //     ir[i][1] = noise;
        // }
        let ir_path = "examples/resource/hall01.wav";
        let convolver_node =
            ConvolverNode::from_file(ir_path, sample_rate, Some(40960)).expect("無法建構 ConvolverNode");
        let drop_count = convolver_node.clone_drop_count();
        let catch_up_count = convolver_node.clone_catch_up_count();

        std::thread::spawn(move || {
            let mut last_drop = 0;
            let mut last_catch_up = 0;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let current_drop = drop_count.load(Ordering::Relaxed);
                let current_catch_up = catch_up_count.load(Ordering::Relaxed);
                
                if current_drop > last_drop || current_catch_up > last_catch_up {
                    println!(
                        "⚠️ Reverb 處理來不及！總掉幀次數: {} (新增: {}), 被追過次數: {} (新增: {})", 
                        current_drop, current_drop - last_drop,
                        current_catch_up, current_catch_up - last_catch_up
                    );
                    last_drop = current_drop;
                    last_catch_up = current_catch_up;
                }
            }
        });

        let convolver = builder.add_node(NodeType::Convolver(convolver_node));

        builder.connect(mic, convolver);

        convolver
    });

    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("🎤 正在播放帶有合成 Reverb 特效的麥克風聲音中...");
    println!("⌨️  按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
