use crate::nodes::NodeType;
use crate::types::{AudioUnit, empty_audio_unit};
use crossbeam_channel::Receiver;
use std::collections::HashMap;
use uuid::Uuid;

/// A unique identifier for a node in the audio graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub Uuid);

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Generic parameter, supporting dynamic updates of node properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeParameter {
    /// Gain/Volume adjustment.
    Gain(f32),
    /// Frequency in Hz.
    Frequency(f32),
    /// Boolean switch (on/off).
    Switch(bool),
    /// Delay time in units (blocks).
    DelayUnits(usize),
    /// Filter cutoff frequency.
    Cutoff(f32),
    /// Filter Q factor (Resonance).
    Q(f32),
}

/// Commands sent to the audio thread to update node parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlMessage {
    /// Sets a parameter on a specific node.
    SetParameter(NodeId, NodeParameter),
}

/// Builder for constructing the audio processing graph.
///
/// `GraphBuilder` allows you to add nodes and define the connections (edges)
/// between them. It supports both standard forward connections and feedback loops.
pub struct GraphBuilder {
    nodes: Vec<NodeType>,
    // edges: [source_node_index] -> [destination_node_index]
    edges: Vec<Vec<usize>>,
    // Feedback edges: (source_node_index, destination_node_index)
    feedback_edges: Vec<(usize, usize)>,
    id_to_index: HashMap<NodeId, usize>,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
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

    /// Adds a node to the graph and returns its unique [`NodeId`].
    pub fn add_node(&mut self, node: NodeType) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.edges.push(Vec::new()); // Initialize an empty output edge list for each node
        let id = NodeId::new();
        self.id_to_index.insert(id, index);
        id
    }

    /// Connects the output of the source node to the input of the destination node.
    pub fn connect(&mut self, source: NodeId, destination: NodeId) {
        if let (Some(&src_idx), Some(&dest_idx)) = (
            self.id_to_index.get(&source),
            self.id_to_index.get(&destination),
        ) {
            self.edges[src_idx].push(dest_idx);
        }
    }

    /// Establishes a feedback connection (back-edge) between nodes.
    ///
    /// Feedback edges are excluded from topological sorting and introduce a 1-block delay.
    /// Use this for feedback loops (e.g., in delays or recursive filters).
    pub fn connect_feedback(&mut self, source: NodeId, destination: NodeId) {
        if let (Some(&src_idx), Some(&dest_idx)) = (
            self.id_to_index.get(&source),
            self.id_to_index.get(&destination),
        ) {
            self.feedback_edges.push((src_idx, dest_idx));
        }
    }

    /// Topological sorting and generation of high-performance StaticGraph with buffer reuse optimization
    pub fn build(
        self,
        destination_id: NodeId,
        msg_receiver: Receiver<ControlMessage>,
    ) -> StaticGraph {
        // 1. Create petgraph for topological sorting (only normal edges, no feedback edges)
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
        // Feedback edges are not added to petgraph, preventing toposort failure from cycles

        let sorted_pet_indices = petgraph::algo::toposort(&pet_graph, None)
            .expect("Audio graph contains a cycle! Use connect_feedback() for feedback loops.");

        let sorted_indices: Vec<usize> = sorted_pet_indices
            .into_iter()
            .map(|idx| idx.index())
            .collect();
        let final_dest_idx = self.id_to_index[&destination_id];

        // 2. Buffer allocation optimization: identify the last usage of each node as an input
        //    Note: source buffers for feedback edges must be preserved until the end (for use in the next frame)
        let mut last_usage = vec![0; self.nodes.len()];
        for (exec_idx, &node_idx) in sorted_indices.iter().enumerate() {
            let mut last_used_at = exec_idx;
            for &dest_idx in &self.edges[node_idx] {
                let dest_exec_idx = sorted_indices.iter().position(|&x| x == dest_idx).unwrap();
                last_used_at = last_used_at.max(dest_exec_idx);
            }
            if node_idx == final_dest_idx {
                last_used_at = usize::MAX; // The buffer for the final destination must be kept until return
            }
            // If this node is a source for any feedback edge, its buffer cannot be reused
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

        // Simulating execution and buffer allocation
        for (exec_idx, &node_idx) in sorted_indices.iter().enumerate() {
            // Acquire from Free List or allocate a new buffer
            let assigned_buffer = if let Some(buf_id) = buffer_free_list.pop() {
                buf_id
            } else {
                let id = next_buffer_id;
                next_buffer_id += 1;
                id
            };
            buffer_assignment[node_idx] = assigned_buffer;
            active_nodes.push(node_idx);

            // Check which nodes are no longer needed, releasing their buffers for reuse
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

        // 3. Reverse edge relationships to: [destination_node] -> Vec<[source_node]>
        let mut inputs_map = vec![Vec::new(); self.nodes.len()];
        for (src_idx, targets) in self.edges.iter().enumerate() {
            for &dest_idx in targets {
                inputs_map[dest_idx].push(src_idx);
            }
        }

        // 4. Create reverse mapping for feedback edges: [destination_node] -> Vec<[source_node]>
        let mut feedback_inputs_map = vec![Vec::new(); self.nodes.len()];
        for &(src_idx, dest_idx) in &self.feedback_edges {
            feedback_inputs_map[dest_idx].push(src_idx);
        }

        // 5. Configure "previous frame" backup buffers for feedback edge sources
        //    prev_frame_buffers: node_idx -> Option<AudioUnit>
        //    Only nodes marked as feedback sources need this
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

/// Fully static, zero-allocation audio runtime core
pub struct StaticGraph {
    nodes: Vec<NodeType>,
    /// Shared and optimized AudioUnit buffers
    node_buffers: Vec<AudioUnit>,
    /// Maps node_idx to its corresponding buffer ID
    buffer_assignment: Vec<usize>,
    /// For each node i, `inputs_map[i]` records its inputs (normal edges)
    inputs_map: Vec<Vec<usize>>,
    /// For each node i, `feedback_inputs_map[i]` records its feedback inputs
    feedback_inputs_map: Vec<Vec<usize>>,
    /// Previous frame backup buffer for feedback source nodes
    prev_frame_buffers: Vec<Option<AudioUnit>>,
    final_destination_index: usize,
    msg_receiver: Receiver<ControlMessage>,
    id_to_index: HashMap<NodeId, usize>,
    /// Correct processing order (determined by topological sort)
    execution_order: Vec<usize>,
}

impl StaticGraph {
    /// Called when CPAL or outer loop requests the next 64-frame chunk
    #[inline(always)]
    pub fn pull_next_unit(&mut self) -> &AudioUnit {
        // 1. Process all non-blocking control messages (e.g., volume changes)
        while let Ok(msg) = self.msg_receiver.try_recv() {
            self.handle_message(msg);
        }

        // 2. Compute nodes following the topological sort order
        // Avoids recursive calls, using a flat for-loop (Cache friendly)
        for &i in &self.execution_order {
            // Combine all input buffers (normal + feedback)
            let mut combined_input = empty_audio_unit();
            let sources = &self.inputs_map[i];
            let feedback_sources = &self.feedback_inputs_map[i];

            let has_input = if sources.is_empty() && feedback_sources.is_empty() {
                false
            } else {
                // Inputs from normal edges
                for &src_idx in sources {
                    let src_buf_idx = self.buffer_assignment[src_idx];
                    let src_buf = &self.node_buffers[src_buf_idx];
                    dasp::slice::add_in_place(&mut combined_input[..], &src_buf[..]);
                }
                // Inputs from feedback edges (reading previous frame's buffer)
                for &src_idx in feedback_sources {
                    if let Some(ref prev_buf) = self.prev_frame_buffers[src_idx] {
                        dasp::slice::add_in_place(&mut combined_input[..], &prev_buf[..]);
                    }
                }
                true
            };

            // Execute node logic and write to its assigned output buffer
            let input_ref = if has_input {
                Some(&combined_input)
            } else {
                None
            };
            let output_buf_idx = self.buffer_assignment[i];
            let output_ref = &mut self.node_buffers[output_buf_idx];

            self.nodes[i].process(input_ref, output_ref);
        }

        // 3. Backup output of all feedback source nodes before returning
        for (node_idx, prev_buf) in self.prev_frame_buffers.iter_mut().enumerate() {
            if let Some(buf) = prev_buf {
                let src_buf_idx = self.buffer_assignment[node_idx];
                buf.copy_from_slice(&self.node_buffers[src_buf_idx]);
            }
        }

        // 4. Return result from the destination node
        let final_buf_idx = self.buffer_assignment[self.final_destination_index];
        &self.node_buffers[final_buf_idx]
    }

    fn handle_message(&mut self, msg: ControlMessage) {
        match msg {
            ControlMessage::SetParameter(node_id, parameter) => {
                if let Some(&index) = self.id_to_index.get(&node_id)
                    && let Some(node) = self.nodes.get_mut(index) {
                        // Enum dispatching (static dispatch)
                        match (node, parameter) {
                            (NodeType::Gain(g), NodeParameter::Gain(val)) => g.set_gain(val),
                            (NodeType::Oscillator(o), NodeParameter::Gain(val)) => o.set_gain(val),
                            (NodeType::Mixer(m), NodeParameter::Gain(val)) => m.set_gain(val),
                            (NodeType::Delay(d), NodeParameter::DelayUnits(val)) => {
                                d.set_delay_units(val)
                            }
                            (NodeType::Filter(f), NodeParameter::Cutoff(val)) => f.set_cutoff(val),
                            (NodeType::Filter(f), NodeParameter::Q(val)) => f.set_q(val),
                            // Dynamic updates for Convolver are not supported (requires IR FFT recalculation)
                            // Implement other property updates here if expanded later
                            // (NodeType::Oscillator(o), NodeParameter::Frequency(val)) => o.set_frequency(val),
                            _ => {}
                        }
                    }
            }
        }
    }
}
