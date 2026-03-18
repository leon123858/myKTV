use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{DelayNode, GainNode, MicrophoneNode, MixerNode, NodeType};
use rust_audio_api::types::AUDIO_UNIT_SIZE;

fn main() {
    // 1. Create Context, which binds to the default output device (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. Static AudioGraph construction (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // Create microphone node
        println!("Creating Microphone node");
        let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to open microphone");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // Dry signal (Vocal volume)
        let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));
        builder.connect(mic, dry_gain);

        // Echo branch (Wet)
        // Delay 130ms (0.13 seconds)
        let delay_time_sec = 0.13;
        let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
        let delay_units = delay_frames / AUDIO_UNIT_SIZE;
        let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE; // Max 2 seconds delay

        println!(
            "Setting echo delay: {} seconds ({} units)",
            delay_time_sec, delay_units
        );
        let delay_node = DelayNode::new(max_delay_units, delay_units);
        let delay = builder.add_node(NodeType::Delay(delay_node));

        // Volume of the delayed sound should be lower than the dry signal
        let wet_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));

        // Add feedback loop: delay output is fed back and attenuated, creating decaying echoes
        let feedback_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));

        builder.connect(mic, delay);
        builder.connect(delay, wet_gain);

        // Connect delay output to feedback_gain, then use feedback edge to connect back to delay
        builder.connect(delay, feedback_gain);
        builder.connect_feedback(feedback_gain, delay);

        // Create Mixer node to combine Dry and Wet signals
        let mixer_node = MixerNode::with_gain(1.0);
        let mixer = builder.add_node(NodeType::Mixer(mixer_node));

        builder.connect(dry_gain, mixer);
        builder.connect(wet_gain, mixer);

        // Master Gain
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.9)));
        builder.connect(mixer, master_gain);

        master_gain
    });

    // 3. Start Audio Thread
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("🎤 Microphone Echo effect active...");
    println!("🗣️  Speak into the microphone to hear 130ms delayed echoes");
    println!("Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
