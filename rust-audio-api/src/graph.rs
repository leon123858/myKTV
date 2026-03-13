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
    Cutoff(f32),
    Q(f32),
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
    // 反饋邊：(source_node_index, destination_node_index)
    feedback_edges: Vec<(usize, usize)>,
    id_to_index: HashMap<NodeId, usize>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            feedback_edges: Vec::new(),
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

    /// 建立反饋連線（back-edge），不參與拓樸排序。
    /// 反饋邊在執行時讀取 source 前一個 frame 的 output buffer，
    /// 引入自然的 1-block 延遲（在音訊反饋路徑中是標準做法）。
    pub fn connect_feedback(&mut self, source: NodeId, destination: NodeId) {
        if let (Some(&src_idx), Some(&dest_idx)) = (
            self.id_to_index.get(&source),
            self.id_to_index.get(&destination),
        ) {
            self.feedback_edges.push((src_idx, dest_idx));
        }
    }

    /// 拓樸排序並生成極致效能的 StaticGraph，並進行 Buffer 重用優化
    pub fn build(
        self,
        destination_id: NodeId,
        msg_receiver: Receiver<ControlMessage>,
    ) -> StaticGraph {
        // 1. 建立 petgraph 以進行拓樸排序（僅使用正常邊，不包含反饋邊）
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
        // 反饋邊不加入 petgraph，這樣 toposort 就不會因為環而失敗

        let sorted_pet_indices = petgraph::algo::toposort(&pet_graph, None)
            .expect("Audio graph contains a cycle! Use connect_feedback() for feedback loops.");

        let sorted_indices: Vec<usize> = sorted_pet_indices
            .into_iter()
            .map(|idx| idx.index())
            .collect();
        let final_dest_idx = self.id_to_index[&destination_id];

        // 2. Buffer 配置優化：找出每個 Node 最後一次被誰當作 Input 使用
        //    注意：反饋邊的 source 需要保留 buffer 到最後（因為下一個 frame 還要用）
        let mut last_usage = vec![0; self.nodes.len()];
        for (exec_idx, &node_idx) in sorted_indices.iter().enumerate() {
            let mut last_used_at = exec_idx;
            for &dest_idx in &self.edges[node_idx] {
                let dest_exec_idx = sorted_indices.iter().position(|&x| x == dest_idx).unwrap();
                last_used_at = last_used_at.max(dest_exec_idx);
            }
            if node_idx == final_dest_idx {
                last_used_at = usize::MAX; // 最終目標的 Buffer 必須保留到最後回傳
            }
            // 如果這個 node 是某條反饋邊的 source，其 buffer 也不能被重用
            for &(fb_src, _) in &self.feedback_edges {
                if fb_src == node_idx {
                    last_used_at = usize::MAX;
                }
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

        // 4. 建立反饋邊的反向映射：[destination_node] -> Vec<[source_node]>
        let mut feedback_inputs_map = vec![Vec::new(); self.nodes.len()];
        for &(src_idx, dest_idx) in &self.feedback_edges {
            feedback_inputs_map[dest_idx].push(src_idx);
        }

        // 5. 為反饋邊的 source 配置 "前一個 frame" 備份 buffer
        //    prev_frame_buffers: node_idx -> Option<AudioUnit>
        //    只有被標記為反饋 source 的節點才需要
        let mut feedback_source_set = vec![false; self.nodes.len()];
        for &(src_idx, _) in &self.feedback_edges {
            feedback_source_set[src_idx] = true;
        }
        let prev_frame_buffers: Vec<Option<AudioUnit>> = feedback_source_set
            .iter()
            .map(|&is_fb| {
                if is_fb {
                    Some(empty_audio_unit())
                } else {
                    None
                }
            })
            .collect();

        StaticGraph {
            nodes: self.nodes,
            node_buffers: buffers,
            buffer_assignment,
            inputs_map,
            feedback_inputs_map,
            prev_frame_buffers,
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
    /// 對於每個節點 i，`inputs_map[i]` 紀錄了誰要當它的 input（正常邊）
    inputs_map: Vec<Vec<usize>>,
    /// 對於每個節點 i，`feedback_inputs_map[i]` 紀錄了誰要透過反饋邊當它的 input
    feedback_inputs_map: Vec<Vec<usize>>,
    /// 反饋邊 source 節點的前一 frame 備份 buffer
    prev_frame_buffers: Vec<Option<AudioUnit>>,
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
            // 合併所有先備 Input Buffer（正常邊 + 反饋邊）
            let mut combined_input = empty_audio_unit();
            let sources = &self.inputs_map[i];
            let feedback_sources = &self.feedback_inputs_map[i];



            let has_input = if sources.is_empty() && feedback_sources.is_empty() {
                false
            } else {
                // 正常邊的 input
                for &src_idx in sources {
                    let src_buf_idx = self.buffer_assignment[src_idx];
                    let src_buf = &self.node_buffers[src_buf_idx];
                    dasp::slice::add_in_place(&mut combined_input[..], &src_buf[..]);
                }
                // 反饋邊的 input（讀取前一 frame 的 buffer）
                for &src_idx in feedback_sources {
                    if let Some(ref prev_buf) = self.prev_frame_buffers[src_idx] {
                        dasp::slice::add_in_place(&mut combined_input[..], &prev_buf[..]);
                    }
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

        // 3. 在回傳前，備份所有反饋 source 節點的 output 到 prev_frame_buffers
        for (node_idx, prev_buf) in self.prev_frame_buffers.iter_mut().enumerate() {
            if let Some(buf) = prev_buf {
                let src_buf_idx = self.buffer_assignment[node_idx];
                buf.copy_from_slice(&self.node_buffers[src_buf_idx]);
            }
        }

        // 4. 回傳 Destination Node 所產生出來的結果
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
                            (NodeType::Delay(d), NodeParameter::DelayUnits(val)) => {
                                d.set_delay_units(val)
                            }
                            (NodeType::Filter(f), NodeParameter::Cutoff(val)) => f.set_cutoff(val),
                            (NodeType::Filter(f), NodeParameter::Q(val)) => f.set_q(val),
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
