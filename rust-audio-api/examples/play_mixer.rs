use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{FileNode, GainNode, MicrophoneNode, MixerNode, NodeType};

fn main() {
    let file_path = "examples/resource/music.mp3";

    // 1. 建立 Context, 其內部已綁定預設輸出設備 (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. 靜態建構 AudioGraph (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // 建立音檔播放節點
        println!("載入音檔: {}", file_path);
        let file_node = FileNode::new(file_path, sample_rate).expect("無法讀取音檔");
        let file = builder.add_node(NodeType::File(file_node));

        // 建立 File 特屬的 Gain 節點 (將音樂調低避免蓋過人聲)
        let file_gain = builder.add_node(NodeType::Gain(GainNode::new(0.1)));
        builder.connect(file, file_gain);

        // 建立麥克風節點來收音
        println!("建立 麥克風 (Microphone) 節點");
        // MicrophoneNode 預設會根據系統麥克風取樣率，自動轉成跟 ctx 相同的 sample_rate
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法開啟麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // 建立 Mixer 節點，用於混合多個輸入，並套用 Limiter (防破音)
        println!("建立 Mixer 節點");
        let mixer_node = MixerNode::with_gain(1.0);
        let mixer = builder.add_node(NodeType::Mixer(mixer_node));

        // 建立 Gain 節點 (作為 Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        // 將 File(經過 Gain 減弱) 和 Microphone 連接到 Mixer 節點
        builder.connect(file_gain, mixer);
        builder.connect(mic, mixer);

        // 將 Mixer 的混音結果送給 Master Gain
        builder.connect(mixer, master_gain);

        // 回傳 Gain 作為圖的最終輸出目的地 (Destination)
        master_gain
    });

    // 3. 啟動 Audio Thread (開始透過 Cpal 輸出聲音)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("正在播放混合音訊 (Music + 麥克風)...");
    println!("按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
