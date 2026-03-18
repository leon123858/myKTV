use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{FileNode, GainNode, MicrophoneNode, MixerNode, NodeType};

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

        // Create specific Gain node for File (lower volume to avoid masking vocals)
        let file_gain = builder.add_node(NodeType::Gain(GainNode::new(0.1)));
        builder.connect(file, file_gain);

        // Create microphone node
        println!("Creating Microphone node");
        // MicrophoneNode scales input to match ctx sample_rate automatically
        let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to open microphone");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // Create Mixer node to combine multiple inputs and apply Limiter
        println!("Creating Mixer node");
        let mixer_node = MixerNode::with_gain(1.0);
        let mixer = builder.add_node(NodeType::Mixer(mixer_node));

        // Create Gain node (as Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        // Connect File (via Gain) and Microphone to Mixer node
        builder.connect(file_gain, mixer);
        builder.connect(mic, mixer);

        // Route Mixer output to Master Gain
        builder.connect(mixer, master_gain);

        // Return Gain as the final output destination of the graph
        master_gain
    });

    // 3. Start Audio Thread (starts outputting sound via CPAL)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("Playing mixed audio (Music + Microphone)...");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
