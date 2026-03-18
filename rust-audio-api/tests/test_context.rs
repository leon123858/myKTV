use rust_audio_api::context::{AudioContext, PerformanceMonitor};
use rust_audio_api::nodes::{NodeType, OscillatorNode};

#[test]
fn test_performance_monitor_default() {
    let monitor = PerformanceMonitor::default();
    assert_eq!(
        monitor
            .late_callbacks
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        monitor
            .current_load_percent
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_audio_context_creation() {
    let ctx_result = AudioContext::new();
    // Tests gracefully handle failure to find device in CI environments
    if let Ok(ctx) = ctx_result {
        assert!(ctx.sample_rate() > 0);
    }
}

#[test]
fn test_build_graph() {
    let mut ctx = match AudioContext::new() {
        Ok(c) => c,
        Err(_) => return, // Skip if no audio device
    };

    let _ = ctx.build_graph(|builder| {
        let osc = OscillatorNode::new(44100.0, 440.0);
        
        builder.add_node(NodeType::Oscillator(osc))
    });
}
