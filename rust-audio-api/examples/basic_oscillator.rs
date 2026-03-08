use rust_audio_api::AudioContext;
use rust_audio_api::nodes::oscillator::OscillatorNode;

fn main() {
    // 1. 建立 Context
    let mut ctx = AudioContext::new().unwrap();

    // 2. 建立一個 440Hz 的正弦波 (A4 音符)
    let sample_rate = ctx.sample_rate() as f64;
    let osc = OscillatorNode::new(sample_rate, 440.0);

    // 3. 連接到目的地並播放
    ctx.connect_destination(Box::new(osc));
    ctx.resume().unwrap();

    println!("播放 440Hz 正弦波中... 按下 Enter 結束");
    let _ = std::io::stdin().read_line(&mut String::new());
}
