# Rust Audio API (rust-audio-api)

一個基於 Rust 開發的音訊處理函式庫，設計靈感來自於 Web Audio API，但針對**最高效能**、**低延遲**以及**系統資源的嚴格控制**進行了深度最佳化。

## 🎯 核心設計目標 (Core Objectives)

為了滿足嚴苛的即時音訊處理需求，本專案堅守以下三大核心原則：

1. **追求最高效能與跨平台低延遲 (Maximum Performance & Low Latency)**
   - 採用 **Pull-Mode (拉取模式)** 的拓撲圖 (Topology) 設計，音訊輸出端主動向圖中提取數據。
   - 硬體交互底層採用 **`cpal` (Cross-Platform Audio Library)**，直接對接作業系統原生的音訊 API (如 WASAPI, ALSA, CoreAudio)，並盡可能利用最高優先權的音訊執行緒 (`audio_thread_priority`) 確保極低延遲。
   - 核心音訊處理迴圈 (Audio Render Thread) 緊密配合 `cpal` 提供的硬體回調 (Hardware Callback)，避免任何不必要的中介層開銷。
   - 內部整合 `dasp` 進行高品質、高效能的即時重採樣 (Resampling) 與聲道轉換。

2. **靜態配置 (Static Configuration & Dispatch)**
   - 不同於 Web Audio API 可以在執行期間動態隨意增刪節點，本專案採用**靜態圖建構 (Static Graph)**。
   - 所有音訊節點 (Nodes) 在單次啟動前透過 `GraphBuilder` 一次性建構並連接完成。
   - 強制使用 Enum (`NodeType`) 達成 Static Dispatch (靜態分發)，徹底避免 `Box<dyn Trait>` 帶來的動態分發 (Dynamic Dispatch) 成本，讓編譯器能進行最大程度的最佳化 (例如 Inline)。

3. **無動態記憶體要求 (Zero Dynamic Allocation in Render Thread)**
   - **音訊執行緒 (Audio Thread) 內絕對禁止動態記憶體分配與鎖 (Locks)**。所有的記憶體分配皆於 `AudioContext` 初始化的設定階段預先完成。
   - 節點間的數據交換與緩衝區管理，使用預先配置的定長陣列或 Lock-free Ring Buffer (`ringbuf`)。
   - 參數更新與跨執行緒通訊 (例如從 UI 調整音量) 使用 Lock-free 傳遞 (`crossbeam-channel`)，保證 Wait-free。

## 🚀 核心架構 (Architecture)

- **AudioContext**: 系統音訊的總管，負責透過 `cpal` 初始化硬體輸出設備 (Default Output Device)、取得取樣率與建立底層的音訊資料流 (Audio Stream)。
- **StaticGraph / GraphBuilder**: 負責將音訊節點連接成有向無環圖 (DAG)。
- **Node 節點系統 (`NodeType`)**:
  - **主動節點 (Active Nodes)**: 透過獨立的機制產生或拉取數據寫入 Ringbuffer，音訊圖從中拉取數據。
    - `FileNode`: 使用 `symphonia` / `hound` 讀取音檔，管理異步的檔案 I/O，並將解碼後的音訊放入 Ringbuffer 供提取。
    - `MicrophoneNode`: 從硬體捕獲麥克風輸入，送入圖中。
    - `OscillatorNode`: 數位訊號生成器。
  - **被動節點 (Passive Nodes)**: 直接處理圖中流動的輸入數據。
    - `GainNode`: 增益(音量)控制機制。

## 💻 使用範例 (Example)

```rust
use rust_audio_api::AudioContext;
use rust_audio_api::nodes::{FileNode, GainNode, NodeType};

fn main() {
    let file_path = "examples/resource/music.mp3";

    // 1. 建立 Context, 其內部已綁定預設輸出設備 (Speaker)
    let mut ctx = AudioContext::new().unwrap();
    let sample_rate = ctx.sample_rate();

    // 2. 靜態建構 AudioGraph (Pull Mode)
    let dest_id = ctx.build_graph(|builder| {
        // 建立音檔播放節點
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

    println!("正在播放音樂中... 按 Enter 結束");
    let _ = std::io::stdin().read_line(&mut String::new());
}
```

## 🛠 給其他 AI 助手的開發提示 (Note for future AI Sessions)

當你在處理此專案的重構、功能新增或 Bug 修復時，請務必嚴格遵守以下限制條件：
1. **Audio Thread 嚴禁 Allocation/Blocking**: 絕對不可在 `AudioUnit::process()` 或音訊圖的熱路徑 (hot path) 中使用 `Vec::new()`, `Box::new()`, `Arc::clone()`, `Mutex::lock()`, `println!()` 等會導致記憶體配置或阻塞的操作。
2. **遵守 Enum 靜態分發**: 保持 `Node` 與 `NodeType` 使用 Enum 結構進行封裝來達成 Static Dispatch，避免引入任何 Trait Object (`dyn Trait`) 導致效能衰退或失去 Inline 最佳化機會。
3. **Lock-Free 跨執行緒通訊**: 任何需要跨執行緒更新的參數（例如主執行緒調整音量），必須使用 Lock-free 結構（如 `crossbeam-channel` 或 `Atomic`）傳遞。
4. **Resampling 統一管理**: 專案已將 Resampler 與頻道數轉換的邏輯抽出為共用的 `ResamplerState` (內部使用 `dasp`)。若需處理新節點的取樣率問題，請沿用既有的 Resampler 模組設計，確保主圖的音訊格式一致性。
