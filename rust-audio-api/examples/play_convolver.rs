use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{ConvolverNode, FileNode, GainNode, NodeType};
use std::sync::atomic::Ordering;
use std::time::Duration;

fn main() {
    let file_path = "examples/resource/music.mp3";

    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    let dest_id = ctx.build_graph(|builder| {
        println!("Loading audio file: {}", file_path);
        let file_node = FileNode::new(file_path, sample_rate).expect("Unable to read audio file");
        let file = builder.add_node(NodeType::File(file_node));

        let ir_path = "examples/resource/hall01.wav";
        println!("Reading IR file: {}", ir_path);

        let convolver_node = ConvolverNode::from_file(ir_path, sample_rate, None)
            .expect("Unable to construct ConvolverNode");

        let drop_count = convolver_node.clone_drop_count();

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                println!("Current Drop Count: {}", drop_count.load(Ordering::Relaxed));
            }
        });

        let convolver = builder.add_node(NodeType::Convolver(convolver_node));

        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        builder.connect(file, convolver);
        builder.connect(convolver, master_gain);

        master_gain
    });

    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("Playing music with multi-echo (Convolver) effect...");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
