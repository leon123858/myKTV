use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{GainNode, MicrophoneNode, NodeType};

fn main() {
    // 1. Create Context, which binds to the default output device (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. Static AudioGraph construction (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // Create microphone node
        println!("Initializing microphone...");
        let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to initialize microphone");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // Create Gain node (as Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));

        // Connect Mic to Gain node
        builder.connect(mic, master_gain);

        // Return Gain as the final output destination of the graph
        master_gain
    });

    // 3. Start Audio Thread (starts outputting sound via CPAL)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("Microphone capturing test...");
    println!("Speak into the microphone; you should hear sound from the speaker!");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
