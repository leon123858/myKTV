use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{GainNode, NodeType, OscillatorNode};

fn main() {
    // 1. 建立 Context
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate() as f64;

    // 2. 靜態建構 AudioGraph (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        let osc = builder.add_node(NodeType::Oscillator(OscillatorNode::new(
            sample_rate,
            440.0,
        )));
        // 為了展示 Parameter，這裡 GainNode 初始設為 0.5
        let gain = builder.add_node(NodeType::Gain(GainNode::new(0.5)));

        builder.connect(osc, gain);

        // 回傳最終的目的地節點
        gain
    });

    // 3. 啟動 Audio Thread
    ctx.resume(dest_id).unwrap();

    println!("播放 440Hz 正弦波中... 按下 Enter 結束");
    let _ = std::io::stdin().read_line(&mut String::new());
}
