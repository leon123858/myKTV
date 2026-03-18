use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{GainNode, NodeType, OscillatorNode};

fn main() {
    // 1. Create Context
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate() as f64;

    // 2. Static AudioGraph construction (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        let osc = builder.add_node(NodeType::Oscillator(OscillatorNode::new(
            sample_rate,
            440.0,
        )));
        // Set GainNode initially to 0.5 to demonstrate parameters
        let gain = builder.add_node(NodeType::Gain(GainNode::new(0.5)));

        builder.connect(osc, gain);

        // Return final destination node
        gain
    });

    // 3. Start Audio Thread
    ctx.resume(dest_id).unwrap();

    println!("Playing 440Hz sine wave... Press Enter to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
}
