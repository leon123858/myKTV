use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{DelayNode, GainNode, MicrophoneNode, MixerNode, NodeType};
use rust_audio_api::types::AUDIO_UNIT_SIZE;

fn main() {
    // 1. 建立 Context, 其內部已綁定預設輸出設備 (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. 靜態建構 AudioGraph (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // 建立麥克風節點來收音
        println!("建立 麥克風 (Microphone) 節點");
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法開啟麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // 原音 (Dry 音量)
        let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));
        builder.connect(mic, dry_gain);

        // Echo 分支 (Wet)
        // 延遲 0.5 秒 (計算 0.5 秒對應多少個 audio units)
        let delay_time_sec = 0.5;
        let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
        let delay_units = delay_frames / AUDIO_UNIT_SIZE;
        let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE; // Max 2 seconds delay

        println!("設定回音延遲: {} 秒 ({} units)", delay_time_sec, delay_units);
        let delay_node = DelayNode::new(max_delay_units, delay_units);
        let delay = builder.add_node(NodeType::Delay(delay_node));

        // 延遲音的音量要比原音低，形成殘響感
        let wet_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));

        builder.connect(mic, delay);
        builder.connect(delay, wet_gain);

        // 建立 Mixer 節點將 Dry 與 Wet 結合
        let mixer_node = MixerNode::with_gain(1.0);
        let mixer = builder.add_node(NodeType::Mixer(mixer_node));
        
        builder.connect(dry_gain, mixer);
        builder.connect(wet_gain, mixer);

        // Master Gain 總音量
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.9)));
        builder.connect(mixer, master_gain);

        master_gain
    });

    // 3. 啟動 Audio Thread
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("🎤 正在播放麥克風 Echo 效果...");
    println!("🗣️  對著麥克風講話會聽到 0.5 秒延遲的回音");
    println!("⌨️  按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
