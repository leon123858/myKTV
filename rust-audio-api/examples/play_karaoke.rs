use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{
    ConvolverNode, DelayNode, FileNode, FilterNode, FilterType, GainNode, MicrophoneNode,
    MixerNode, NodeType,
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

/// 卡拉 OK 範例 — 即時麥克風 + 背景音樂 + 延遲回音(含反饋迴路) + 合成殘響
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
        let music_gain = builder.add_node(NodeType::Gain(GainNode::new(0.15)));
        builder.connect(file, music_gain);

        // ── 麥克風 ──
        println!("建立麥克風節點");
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法開啟麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // ── Filters1: BandPass 麥克風存在感濾波（200-6000 Hz）──
        // Q = 0.7 讓 Q 值更集中在人聲頻段
        let filters1 = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::BandPass,
            sample_rate,
            1000.0,
            0.7,
        )));
        builder.connect(mic, filters1);

        let mic_gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));
        builder.connect(filters1, mic_gain);

        // ── Dry (原音) ──
        let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.7)));
        builder.connect(mic_gain, dry_gain);

        // ── Echo (延遲回音 + 反饋迴路) ──
        let delay_time_sec = 0.08; // 縮短為 80ms，產生更紮實的 KTV 效果
        let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
        let delay_units = delay_frames / AUDIO_UNIT_SIZE;
        let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE;
        println!("回音延遲: {}s ({} units)", delay_time_sec, delay_units);

        // Filters2: LowPass 回音反饋變暗濾波（2500 Hz）讓反饋更溫潤
        let filters2 = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::LowPass,
            sample_rate,
            2500.0,
            0.707,
        )));

        let delay = builder.add_node(NodeType::Delay(DelayNode::new(
            max_delay_units,
            delay_units,
        )));
        let echo_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));

        // 正常路徑：mic_gain → delay → echo_gain
        builder.connect(mic_gain, delay);
        builder.connect(delay, echo_gain);

        // 反饋路徑：echo_gain → filters2 → delay（使用 feedback 連線）
        builder.connect_feedback(echo_gain, filters2);
        builder.connect(filters2, delay);

        // ── Reverb (合成殘響) ──
        let ir_path = "examples/resource/plate01.wav";
        let convolver_node =
            ConvolverNode::from_file(ir_path, sample_rate, None).expect("無法建構 ConvolverNode");
        let convolver = builder.add_node(NodeType::Convolver(convolver_node));
        let reverb_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));
        builder.connect(mic_gain, convolver);
        builder.connect(convolver, reverb_gain);

        // ── 總混音 (Compressor = MixerNode with gain + clipping) ──
        let compressor = builder.add_node(NodeType::Mixer(MixerNode::with_gain(0.8)));
        builder.connect(music_gain, compressor);
        builder.connect(dry_gain, compressor);
        builder.connect(echo_gain, compressor);
        builder.connect(reverb_gain, compressor);

        compressor
    });

    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("🎤 卡拉 OK 模式啟動！");
    println!("🎵 背景音樂 + 麥克風即時回音(含反饋) + 合成殘響");
    println!("🎛️  Dry: 0.7 / Echo: 0.4 (0.08s feedback) / Reverb: 0.4");
    println!("🔊 filters1: BandPass 1000Hz / filters2: LowPass 2500Hz");
    println!("⌨️  按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
