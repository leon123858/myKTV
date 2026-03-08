pub mod context;
pub mod graph;
pub mod nodes;
pub mod types;

pub use context::AudioContext;
pub use graph::{GraphBuilder, NodeId, StaticGraph};
pub use types::{AUDIO_UNIT_SIZE, AudioUnit};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
