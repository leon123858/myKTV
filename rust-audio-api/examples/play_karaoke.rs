use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{
    ConvolverNode, DelayNode, FileNode, FilterNode, FilterType, GainNode, MicrophoneNode,
    MixerNode, NodeType,
};
use rust_audio_api::types::AUDIO_UNIT_SIZE;
use std::sync::atomic::Ordering;
use std::time::Duration;
/// Generates a synthetic IR suitable for Karaoke (exponentially decaying noise + early reflections)
fn generate_karaoke_ir(sample_rate: u32) -> Vec<[f32; 2]> {
    let duration_sec = 0.08; // 80ms small room reverb
    let len = (sample_rate as f32 * duration_sec) as usize;
    let mut ir = vec![[0.0f32; 2]; len];

    // Simple LCG pseudo-random generator (avoids adding rand crate)
    let mut seed: u32 = 12345;
    let mut rand_f32 = || -> f32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((seed >> 16) as f32 / 32768.0) - 1.0 // Range [-1, 1]
    };

    // 早期反射 (discrete echoes)
    let early_reflections: &[(usize, f32)] = &[
        (0, 1.0),                                      // Direct
        ((0.005 * sample_rate as f64) as usize, 0.6),  // 5ms
        ((0.012 * sample_rate as f64) as usize, 0.4),  // 12ms
        ((0.020 * sample_rate as f64) as usize, 0.25), // 20ms
        ((0.031 * sample_rate as f64) as usize, 0.15), // 31ms
    ];

    for &(offset, gain) in early_reflections {
        if offset < len {
            ir[offset] = [gain, gain];
        }
    }

    // Exponentially decaying diffuse tail (starting from 10ms)
    let tail_start = (0.010 * sample_rate as f64) as usize;
    let decay_rate = 6.0 / duration_sec as f64; // T60 ≈ duration

    for (i, ir) in ir.iter_mut().enumerate().take(len).skip(tail_start) {
        let t = i as f64 / sample_rate as f64;
        let envelope = (-decay_rate * t).exp() as f32 * 0.3;
        let noise_l = rand_f32() * envelope;
        let noise_r = rand_f32() * envelope;
        ir[0] += noise_l;
        ir[1] += noise_r;
    }

    ir
}

/// Karaoke Example — Real-time Microphone + Background Music + Echo (with feedback loop) + Synthetic Reverb
///
/// Audio Graph:
///
///   Player → MusicGain ────────────────────────────────────────────┐
///                                                                  ↓
///   Mic → Filters1(BP) → MicGain ─┬─ (dry) → DryGain ──────────→ Compressor(Mixer) → Speaker
///                                  │                               ↑   ↑
///                                  ├─ Delay → EchoGain ───────────┘   │
///                                  │    ↑                              │
///                                  │    └─ Filters2(LP) ←────┘ (feedback from EchoGain)
///                                  │                                   │
///                                  └─ Convolver → ReverbGain ─────────┘
///
fn main() {
    let music_path = "examples/resource/music.mp3";

    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // Statically generate IR
    let ir = generate_karaoke_ir(sample_rate);
    println!(
        "Synthetic IR: {} samples ({:.1}ms)",
        ir.len(),
        ir.len() as f32 / sample_rate as f32 * 1000.0
    );

    let dest_id = ctx.build_graph(|builder| {
        // -- Background Music --
        println!("Loading background music: {}", music_path);
        let file_node = FileNode::new(music_path, sample_rate).expect("Unable to read audio file");
        let file = builder.add_node(NodeType::File(file_node));
        let music_gain = builder.add_node(NodeType::Gain(GainNode::new(0.15)));
        builder.connect(file, music_gain);

        // -- Microphone --
        println!("Creating microphone node");
        let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to open microphone");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // -- Filters1: HighPass to reduce low-frequency noise/pops (80 Hz) --
        // Removes excess low-frequency from the mic for a cleaner vocal
        let filters1 = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::HighPass,
            sample_rate,
            80.0,
            0.707,
        )));
        builder.connect(mic, filters1);

        let mic_gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));
        builder.connect(filters1, mic_gain);

        // -- Dry (Vocals) --
        let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.7)));
        builder.connect(mic_gain, dry_gain);

        // -- Echo (Delay + Feedback Loop) --
        let delay_time_sec = 0.22; // 220ms is common for Asian Karaoke systems
        let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
        let delay_units = delay_frames / AUDIO_UNIT_SIZE;
        let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE;
        println!("Echo delay: {}s ({} units)", delay_time_sec, delay_units);

        // Filters2: LowPass to darken echo feedback (3500 Hz) for a smoother sound
        let filters2 = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::LowPass,
            sample_rate,
            3500.0,
            0.707,
        )));

        let delay = builder.add_node(NodeType::Delay(DelayNode::new(
            max_delay_units,
            delay_units,
        )));
        let echo_gain = builder.add_node(NodeType::Gain(GainNode::new(0.45)));

        // Normal path: mic_gain -> delay -> echo_gain
        builder.connect(mic_gain, delay);
        builder.connect(delay, echo_gain);

        // Feedback path: echo_gain -> filters2 -> delay (using feedback connection)
        builder.connect_feedback(echo_gain, filters2);
        builder.connect(filters2, delay);

        // -- Reverb (Convolution Reverb) --
        let ir_path = "examples/resource/plate01.wav";
        let max_reverb_len = Some((sample_rate as f32 * 1.2) as usize); // Limit length to 1.2 seconds
        let config = rust_audio_api::nodes::ConvolverConfig::default();
        
        let convolver_node =
            ConvolverNode::from_file_with_config(ir_path, sample_rate, max_reverb_len, config)
                .expect("Unable to construct ConvolverNode");
        let convolver = builder.add_node(NodeType::Convolver(convolver_node));
        let reverb_gain = builder.add_node(NodeType::Gain(GainNode::new(0.25))); // Lower Reverb volume
        builder.connect(mic_gain, convolver);
        builder.connect(convolver, reverb_gain);

        // -- Final Mix (Compressor = MixerNode with gain + clipping) --
        let compressor = builder.add_node(NodeType::Mixer(MixerNode::with_gain(0.8)));
        builder.connect(music_gain, compressor);
        builder.connect(dry_gain, compressor);
        builder.connect(echo_gain, compressor);
        builder.connect(reverb_gain, compressor);

        compressor
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
                println!(
                    "⚠️ Audio thread load too high! Current CPU load: {}%",
                    load_percent
                );
            }
        }
    });

    println!("========================================");
    println!("🎤 Karaoke mode started!");
    println!("🎵 Background Music + Real-time Echo (with feedback) + Reverb");
    println!("🎛️  Dry: 0.7 / Echo: 0.45 (0.22s) / Reverb: 0.25 (1.2s)");
    println!("🔊 Filters1: HighPass 80Hz / Filters2: LowPass 3500Hz");
    println!("⌨️  Press Enter to exit...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
