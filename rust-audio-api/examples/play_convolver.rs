use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{ConvolverConfig, ConvolverNode, FileNode, GainNode, NodeType};

fn main() {
    let file_path = "examples/resource/music.mp3";

    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    let dest_id = ctx.build_graph(|builder| {
        println!("載入音檔: {}", file_path);
        let file_node = FileNode::new(file_path, sample_rate).expect("無法讀取音檔");
        let file = builder.add_node(NodeType::File(file_node));

        let ir_path = "examples/resource/hall01.wav";
        println!("讀取 IR 檔案: {}", ir_path);

        let config = ConvolverConfig {
            stereo: false,
            growth_exponent: 4,
            base_block_multiplier: 1,
        };
        let convolver_node = ConvolverNode::from_file_with_config(ir_path, Some(512), config)
            .expect("無法建構 ConvolverNode");

        let convolver = builder.add_node(NodeType::Convolver(convolver_node));

        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        builder.connect(file, convolver);
        builder.connect(convolver, master_gain);

        master_gain
    });

    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("正在播放帶有多重回聲(Convolver)特效的音樂中...");
    println!("按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
