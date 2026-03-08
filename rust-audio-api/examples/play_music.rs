use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{FileNode, GainNode, NodeType};

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

        // 建立 Gain 節點 (作為 Master Volume)
        let master_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));

        // 將 File 連接到 Gain 節點
        builder.connect(file, master_gain);

        // 回傳 Gain 作為圖的最終輸出目的地 (Destination)
        master_gain
    });

    // 3. 啟動 Audio Thread (開始透過 Cpal 輸出聲音)
    ctx.resume(dest_id).unwrap();

    println!("========================================");
    println!("正在播放音樂中...");
    println!("按下 Enter 鍵結束程式...");
    println!("========================================");

    let _ = std::io::stdin().read_line(&mut String::new());
}
