use crate::nodes::NodeType;
use crate::types::{AudioUnit, empty_audio_unit};
use crossbeam_channel::Receiver;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// 泛型化參數，支援動態修訂節點的各種屬性
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeParameter {
    Gain(f32),
    Frequency(f32),
    Switch(bool),
    DelayUnits(usize),
    // Play, Stop, etc.
}

/// UI 或 Main Thread 用來發送給 Audio Thread 的指令。
/// 由於不支援拓樸改變，這裡只能發送「參數修改」指令。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlMessage {
    SetParameter(NodeId, NodeParameter),
}

pub struct GraphBuilder {
    nodes: Vec<NodeType>,
    // edges: [source_node_index] -> [destination_node_index]
    edges: Vec<Vec<usize>>,
    id_to_index: HashMap<NodeId, usize>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            id_to_index: HashMap::new(),
        }
    }

    /// 加入一個節點，回傳它的唯一 ID
    pub fn add_node(&mut self, node: NodeType) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.edges.push(Vec::new()); // 每一個節點初始化一個空的 output 邊列表
        let id = NodeId::new();
        self.id_to_index.insert(id, index);
        id
    }

    /// 將 source 接到 destination 的上方 (source 是 dest 的 input)
    pub fn connect(&mut self, source: NodeId, destination: NodeId) {
        if let (Some(&src_idx), Some(&dest_idx)) = (
            self.id_to_index.get(&source),
            self.id_to_index.get(&destination),
        ) {
            self.edges[src_idx].push(dest_idx);
        }
    }

    /// 拓樸排序並生成極致效能的 StaticGraph
    pub fn build(
        self,
        destination_id: NodeId,
        msg_receiver: Receiver<ControlMessage>,
    ) -> StaticGraph {
        // 為了每個 Node 配置「專屬的 Output Buffer」，確保互相不會覆蓋
        let buffers_count = self.nodes.len();
        let mut buffers = Vec::with_capacity(buffers_count);
        for _ in 0..buffers_count {
            buffers.push(empty_audio_unit());
        }

        // 把 edge 關係反轉，變成：[destination_node] -> Vec<[source_node]>
        // 這樣要算某個 dest 的時候，就知道要去拉 (pull) 哪些 source 的 buffer 作為 input 疊加
        let mut inputs_map = vec![Vec::new(); self.nodes.len()];
        for (src_idx, targets) in self.edges.iter().enumerate() {
            for &dest_idx in targets {
                inputs_map[dest_idx].push(src_idx);
            }
        }

        let final_dest_idx = self.id_to_index[&destination_id];

        StaticGraph {
            nodes: self.nodes,
            node_output_buffers: buffers,
            inputs_map,
            final_destination_index: final_dest_idx,
            msg_receiver,
            id_to_index: self.id_to_index,
        }
    }
}

/// 完全靜態、零記憶體配置的音訊運行核心
pub struct StaticGraph {
    nodes: Vec<NodeType>,
    /// 存放每個節點剛計算完成的 64-frame AudioUnit 資料
    node_output_buffers: Vec<AudioUnit>,
    /// 對於每個節點 i，`inputs_map[i]` 紀錄了誰要當它的 input
    inputs_map: Vec<Vec<usize>>,
    final_destination_index: usize,
    msg_receiver: Receiver<ControlMessage>,
    id_to_index: HashMap<NodeId, usize>,
}

impl StaticGraph {
    /// 當 CPAL 或外層迴圈要求要下一個 64-frame chunks，就呼叫這裡
    #[inline(always)]
    pub fn pull_next_unit(&mut self) -> &AudioUnit {
        // 1. 先處理所有非阻塞的控制訊息 (如改音量)
        while let Ok(msg) = self.msg_receiver.try_recv() {
            self.handle_message(msg);
        }

        // 2. 依次計算每個 Node (前提是此處 nodes 為已拓樸排序，索引由小到大皆為 safe 的評估順序)
        // 這裡不用遞迴回頭 Call，而是平坦的 For 迴圈 (Cache 極度友好)
        for i in 0..self.nodes.len() {
            // 合併所有先備 Input Buffer
            let mut combined_input = empty_audio_unit();
            let sources = &self.inputs_map[i];

            let is_mixer = matches!(self.nodes[i], NodeType::Mixer(_));
            assert!(
                is_mixer || sources.len() <= 1,
                "只有 MixerNode 可以接受多個輸入！Node {} (型別非 Mixer) 卻收到了 {} 個輸入。",
                i,
                sources.len()
            );

            let has_input = if sources.is_empty() {
                false
            } else {
                for &src_idx in sources {
                    // 將 src 的 output 疊加到 combined_input，利用 dasp 高效處理混音加總
                    let src_buf = &self.node_output_buffers[src_idx];
                    dasp::slice::add_in_place(&mut combined_input[..], &src_buf[..]);
                }
                true
            };

            // 執行 Node 邏輯並寫入其專屬 Output Buffer
            let input_ref = if has_input {
                Some(&combined_input)
            } else {
                None
            };
            let output_ref = &mut self.node_output_buffers[i];

            self.nodes[i].process(input_ref, output_ref);
        }

        // 3. 回傳 Destination Node 所產生出來的結果
        &self.node_output_buffers[self.final_destination_index]
    }

    fn handle_message(&mut self, msg: ControlMessage) {
        match msg {
            ControlMessage::SetParameter(node_id, parameter) => {
                if let Some(&index) = self.id_to_index.get(&node_id) {
                    if let Some(node) = self.nodes.get_mut(index) {
                        // Enum dispatching (靜態派發)
                        match (node, parameter) {
                            (NodeType::Gain(g), NodeParameter::Gain(val)) => g.set_gain(val),
                            (NodeType::Oscillator(o), NodeParameter::Gain(val)) => o.set_gain(val),
                            (NodeType::Mixer(m), NodeParameter::Gain(val)) => m.set_gain(val),
                            (NodeType::Delay(d), NodeParameter::DelayUnits(val)) => d.set_delay_units(val),
                            // 若後續擴充別的屬性可以在這裡實作
                            // (NodeType::Oscillator(o), NodeParameter::Frequency(val)) => o.set_frequency(val),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
