use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{
    ConvolverConfig, ConvolverNode, DelayNode, FileNode, GainNode, MicrophoneNode, MixerNode,
    NodeType,
};
use rust_audio_api::types::AUDIO_UNIT_SIZE;

/// 生成適合卡拉 OK 的合成 IR（指數衰減噪聲 + 早期反射）
fn generate_karaoke_ir(sample_rate: u32) -> Vec<[f32; 2]> {
    let duration_sec = 0.08; // 80ms 小房間殘響
    let len = (sample_rate as f32 * duration_sec) as usize;
    let mut ir = vec![[0.0f32; 2]; len];

    // 簡易 LCG 偽隨機產生器（避免引入 rand crate）
    let mut seed: u32 = 12345;
    let mut rand_f32 = || -> f32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((seed >> 16) as f32 / 32768.0) - 1.0 // 範圍 [-1, 1]
    };

    // 早期反射 (discrete echoes)
    let early_reflections: &[(usize, f32)] = &[
        (0, 1.0),                                      // 直達
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

    // 指數衰減漫射尾（從 10ms 開始）
    let tail_start = (0.010 * sample_rate as f64) as usize;
    let decay_rate = 6.0 / duration_sec as f64; // T60 ≈ duration

    for i in tail_start..len {
        let t = i as f64 / sample_rate as f64;
        let envelope = (-decay_rate * t).exp() as f32 * 0.3;
        let noise_l = rand_f32() * envelope;
        let noise_r = rand_f32() * envelope;
        ir[i][0] += noise_l;
        ir[i][1] += noise_r;
    }

    ir
}

/// 卡拉 OK 範例 — 即時麥克風 + 背景音樂 + 延遲回音 + 合成殘響
///
/// Audio Graph:
///
///   File ──→ FileGain ──────────────────────────────┐
///                                                   ↓
///   Mic ──→ MicGain ──┬─ (dry) ──→ DryGain ──────→ Mixer ──→ MasterGain ──→ Speaker
///                     │                             ↑   ↑
///                     ├─ Delay ──→ EchoGain ────────┘   │
///                     │                                  │
///                     └─ Convolver ──→ ReverbGain ───────┘
///
fn main() {
    let music_path = "examples/resource/music.mp3";

    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 靜態生成 IR
    let ir = generate_karaoke_ir(sample_rate);
    println!(
        "合成 IR: {} samples ({:.1}ms)",
        ir.len(),
        ir.len() as f32 / sample_rate as f32 * 1000.0
    );

    let dest_id = ctx.build_graph(|builder| {
        // ── 背景音樂 ──
        println!("載入背景音樂: {}", music_path);
        let file_node = FileNode::new(music_path, sample_rate).expect("無法讀取音檔");
        let file = builder.add_node(NodeType::File(file_node));
        let file_gain = builder.add_node(NodeType::Gain(GainNode::new(0.15)));
        builder.connect(file, file_gain);

        // ── 麥克風 ──
        println!("建立麥克風節點");
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法開啟麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));
        let mic_gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));
        builder.connect(mic, mic_gain);

        // ── Dry (原音) ──
        let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.7)));
        builder.connect(mic_gain, dry_gain);

        // ── Echo (延遲回音) ──
        let delay_time_sec = 0.15;
        let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
        let delay_units = delay_frames / AUDIO_UNIT_SIZE;
        let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE;
        println!("回音延遲: {}s ({} units)", delay_time_sec, delay_units);

        let delay = builder.add_node(NodeType::Delay(DelayNode::new(
            max_delay_units,
            delay_units,
        )));
        let echo_gain = builder.add_node(NodeType::Gain(GainNode::new(0.35)));
        builder.connect(mic_gain, delay);
        builder.connect(delay, echo_gain);

        // ── Reverb (合成殘響) ──
        let reverb_config = ConvolverConfig {
            stereo: false,
            growth_exponent: 4,
            base_block_multiplier: 1,
        };
        let convolver_node = ConvolverNode::with_config(&ir, reverb_config);
        let convolver = builder.add_node(NodeType::Convolver(convolver_node));
        let reverb_gain = builder.add_node(NodeType::Gain(GainNode::new(0.3)));
        builder.connect(mic_gain, convolver);
        builder.connect(convolver, reverb_gain);

        // ── 總混音 ──
        let mixer = builder.add_node(NodeType::Mixer(MixerNode::with_gain(1.0)));
        builder.connect(file_gain, mixer);
        builder.connect(dry_gain, mixer);
        builder.connect(echo_gain, mixer);
        builder.connect(reverb_gain, mixer);

        // ── Master Gain ──
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));
        builder.connect(mixer, master_gain);

        master_gain
    });

    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("🎤 卡拉 OK 模式啟動！");
    println!("🎵 背景音樂 + 麥克風即時回音 + 合成殘響");
    println!("🎛️  Dry: 0.7 / Echo: 0.35 (0.15s) / Reverb: 0.3");
    println!("⌨️  按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
