use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{GainNode, MicrophoneNode, NodeType};

fn main() {
    // 1. 建立 Context, 其內部已綁定預設輸出設備 (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    println!("AudioContext initialized with sample rate: {}", sample_rate);

    // 2. 靜態建構 AudioGraph (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // 建立麥克風節點
        println!("初始化麥克風...");
        let mic_node = MicrophoneNode::new(sample_rate).expect("無法初始化麥克風");
        let mic = builder.add_node(NodeType::Microphone(mic_node));

        // 建立 Gain 節點 (作為 Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));

        // 將 Mic 連接到 Gain 節點
        builder.connect(mic, master_gain);

        // 回傳 Gain 作為圖的最終輸出目的地 (Destination)
        master_gain
    });

    // 3. 啟動 Audio Thread (開始透過 Cpal 輸出聲音)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("麥克風收音測試中...");
    println!("對著麥克風講話，應該可以聽到聲音由喇叭輸出！");
    println!("按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
