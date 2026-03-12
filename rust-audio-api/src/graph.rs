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

    /// 拓樸排序並生成極致效能的 StaticGraph，並進行 Buffer 重用優化
    pub fn build(
        self,
        destination_id: NodeId,
        msg_receiver: Receiver<ControlMessage>,
    ) -> StaticGraph {
        // 1. 建立 petgraph 以進行拓樸排序
        let mut pet_graph = petgraph::graph::DiGraph::<(), ()>::new();
        let mut pet_indices = Vec::with_capacity(self.nodes.len());
        
        for _ in 0..self.nodes.len() {
            pet_indices.push(pet_graph.add_node(()));
        }

        for (src, targets) in self.edges.iter().enumerate() {
            for &dest in targets {
                pet_graph.add_edge(pet_indices[src], pet_indices[dest], ());
            }
        }

        let sorted_pet_indices = petgraph::algo::toposort(&pet_graph, None)
            .expect("Audio graph contains a cycle! Feedback loops are not supported yet.");

        let sorted_indices: Vec<usize> = sorted_pet_indices.into_iter().map(|idx| idx.index()).collect();
        let final_dest_idx = self.id_to_index[&destination_id];

        // 2. Buffer 配置優化：找出每個 Node 最後一次被誰當作 Input 使用，當過了那個時候，其 Output Buffer 就能重用
        let mut last_usage = vec![0; self.nodes.len()];
        for (exec_idx, &node_idx) in sorted_indices.iter().enumerate() {
            let mut last_used_at = exec_idx;
            for &dest_idx in &self.edges[node_idx] {
                // dest_idx 也是一定存在於 sorted_indices 中的
                let dest_exec_idx = sorted_indices.iter().position(|&x| x == dest_idx).unwrap();
                last_used_at = last_used_at.max(dest_exec_idx);
            }
            if node_idx == final_dest_idx {
                last_used_at = usize::MAX; // 最終目標的 Buffer 必須保留到最後回傳
            }
            last_usage[node_idx] = last_used_at;
        }

        let mut buffer_assignment = vec![0; self.nodes.len()];
        let mut buffer_free_list = Vec::new();
        let mut next_buffer_id = 0;
        let mut active_nodes = Vec::new();

        // 模擬執行並分配 Buffer
        for (exec_idx, &node_idx) in sorted_indices.iter().enumerate() {
            // 從 Free List 拿，或者宣告新的 Buffer
            let assigned_buffer = if let Some(buf_id) = buffer_free_list.pop() {
                buf_id
            } else {
                let id = next_buffer_id;
                next_buffer_id += 1;
                id
            };
            buffer_assignment[node_idx] = assigned_buffer;
            active_nodes.push(node_idx);
            
            // 檢查哪些 Node 的使命已達標，可將它們的 Buffer 釋出重用
            active_nodes.retain(|&active_node| {
                if last_usage[active_node] == exec_idx {
                    buffer_free_list.push(buffer_assignment[active_node]);
                    false
                } else {
                    true
                }
            });
        }

        let buffers_count = next_buffer_id;
        let buffers = vec![empty_audio_unit(); buffers_count];

        // 3. 把 edge 關係反轉，變成：[destination_node] -> Vec<[source_node]>
        let mut inputs_map = vec![Vec::new(); self.nodes.len()];
        for (src_idx, targets) in self.edges.iter().enumerate() {
            for &dest_idx in targets {
                inputs_map[dest_idx].push(src_idx);
            }
        }

        StaticGraph {
            nodes: self.nodes,
            node_buffers: buffers,
            buffer_assignment,
            inputs_map,
            final_destination_index: final_dest_idx,
            msg_receiver,
            id_to_index: self.id_to_index,
            execution_order: sorted_indices,
        }
    }
}

/// 完全靜態、零記憶體配置的音訊運行核心
pub struct StaticGraph {
    nodes: Vec<NodeType>,
    /// 共用優化後的 AudioUnit Buffers
    node_buffers: Vec<AudioUnit>,
    /// 紀錄 node_idx 對應到哪個 buffer id
    buffer_assignment: Vec<usize>,
    /// 對於每個節點 i，`inputs_map[i]` 紀錄了誰要當它的 input
    inputs_map: Vec<Vec<usize>>,
    final_destination_index: usize,
    msg_receiver: Receiver<ControlMessage>,
    id_to_index: HashMap<NodeId, usize>,
    /// 節點正確的計算順序（由拓樸排序決定）
    execution_order: Vec<usize>,
}

impl StaticGraph {
    /// 當 CPAL 或外層迴圈要求要下一個 64-frame chunks，就呼叫這裡
    #[inline(always)]
    pub fn pull_next_unit(&mut self) -> &AudioUnit {
        // 1. 先處理所有非阻塞的控制訊息 (如改音量)
        while let Ok(msg) = self.msg_receiver.try_recv() {
            self.handle_message(msg);
        }

        // 2. 依照拓樸排序的安全評估順序來計算每個 Node
        // 這裡不用遞迴回頭 Call，而是平坦的 For 迴圈 (Cache 極度友好)
        for &i in &self.execution_order {
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
                    // 根據 buffer_assignment 找到這個 source 存放資料的真實 buffer
                    let src_buf_idx = self.buffer_assignment[src_idx];
                    let src_buf = &self.node_buffers[src_buf_idx];
                    dasp::slice::add_in_place(&mut combined_input[..], &src_buf[..]);
                }
                true
            };

            // 執行 Node 邏輯並寫入其專屬 (或者重用分配的) Output Buffer
            let input_ref = if has_input {
                Some(&combined_input)
            } else {
                None
            };
            let output_buf_idx = self.buffer_assignment[i];
            let output_ref = &mut self.node_buffers[output_buf_idx];

            self.nodes[i].process(input_ref, output_ref);
        }

        // 3. 回傳 Destination Node 所產生出來的結果
        let final_buf_idx = self.buffer_assignment[self.final_destination_index];
        &self.node_buffers[final_buf_idx]
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
                            // Convolver 參數動態更新不支援（因需要重算 IR FFT），暫時不處理
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
