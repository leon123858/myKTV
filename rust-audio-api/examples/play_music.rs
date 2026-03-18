use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{FileNode, GainNode, NodeType};

fn main() {
    let file_path = "examples/resource/music.mp3";

    // 1. Create Context, which binds to the default output device (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. Static AudioGraph construction (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // Create audio file playback node
        println!("Loading audio file: {}", file_path);
        let file_node = FileNode::new(file_path, sample_rate).expect("Unable to read audio file");
        let file = builder.add_node(NodeType::File(file_node));

        // Create Gain node (as Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        // Connect File to Gain node
        builder.connect(file, master_gain);

        // Return Gain as the final output destination of the graph
        master_gain
    });

    // 3. Start Audio Thread (starts outputting sound via CPAL)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("Playing music...");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
