use rust_audio_api::AudioContext;
use rust_audio_api::graph::{ControlMessage, NodeParameter};
use rust_audio_api::nodes::{FilterNode, FilterType, GainNode, NodeType, OscillatorNode};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), anyhow::Error> {
    println!("Starting Dynamic Properties Example...");
    let mut ctx = AudioContext::new()?;
    let sample_rate = ctx.sample_rate();

    let mut filter_id = None;
    let mut gain_id = None;

    let dest_id = ctx.build_graph(|builder| {
        // Create an oscillator (use 110Hz for a nice bassy sound that filters well)
        let osc = builder.add_node(NodeType::Oscillator(OscillatorNode::new(
            sample_rate as f64,
            110.0,
        )));

        // Create a LowPass filter with an initial cutoff of 100Hz and high resonance (Q)
        let filter = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::LowPass,
            sample_rate,
            100.0,
            2.0, // High Q for a pronounced filter sweep effect
        )));
        filter_id = Some(filter);

        // Create a Gain node to control overall volume and avoid clipping
        let gain_node = GainNode::new(0.5);
        let gain = builder.add_node(NodeType::Gain(gain_node));
        gain_id = Some(gain);

        // Connect the nodes: Oscillator -> Filter -> Gain -> Destination
        builder.connect(osc, filter);
        builder.connect(filter, gain);

        gain
    });

    // Start audio playback
    ctx.resume(dest_id)?;
    println!("Audio context resumed. Playing low tone...");

    let sender = ctx.control_sender();
    let filter_id = filter_id.unwrap();
    let gain_id = gain_id.unwrap();

    let start_time = Instant::now();
    let duration = Duration::from_secs(10);

    println!("Sweeping filter cutoff and gain dynamically...");

    // Loop for 10 seconds, dynamically updating the parameters
    while start_time.elapsed() < duration {
        let elapsed_secs = start_time.elapsed().as_secs_f32();

        // 1. Sweep filter cutoff between 100Hz and 3000Hz using a sine wave pattern over time
        //    (elapsed_secs * 2.0) controls the speed of the sweep
        let sweep_phase = elapsed_secs * 1.5;
        let sweep_normalized = (sweep_phase.sin() + 1.0) / 2.0; // 0.0 to 1.0
        let current_cutoff = 100.0 + sweep_normalized * 2900.0;

        // Send the control message to update the filter's cutoff frequency
        let _ = sender.send(ControlMessage::SetParameter(
            filter_id,
            NodeParameter::Cutoff(current_cutoff),
        ));

        // 2. Modulate the gain slightly to add a tremolo effect
        //    (elapsed_secs * 8.0) is a faster sine wave
        let tremolo_phase = elapsed_secs * 8.0;
        let tremolo_normalized = (tremolo_phase.sin() + 1.0) / 2.0;
        let current_gain = 0.3 + tremolo_normalized * 0.4; // 0.3 to 0.7

        // Send the control message to update the gain
        let _ = sender.send(ControlMessage::SetParameter(
            gain_id,
            NodeParameter::Gain(current_gain),
        ));

        // Sleep briefly to avoid overwhelming the message queue
        thread::sleep(Duration::from_millis(10));
    }

    println!("Finished example playback.");
    Ok(())
}
