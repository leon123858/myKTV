# Rust Audio API (rust-audio-api)

An audio processing library developed in Rust, inspired by the Web Audio API but deeply optimized for **maximum performance**, **low latency**, and **strict control over system resources**. This library is particularly suited for applications requiring real-time audio processing, such as karaoke systems, real-time effects processors, and interactive audio applications.

## 🎯 Core Objectives

To meet the rigorous demands of real-time audio processing, this project strictly adheres to three core principles:

1. **Maximum Performance & Low Latency**
   - Utilizes a **Pull-Mode** topology design, where the audio output device actively pulls data from the graph.
   - The underlying hardware interaction relies on **`cpal` (Cross-Platform Audio Library)** to directly interface with native OS audio APIs (WASAPI, ALSA, CoreAudio), leveraging high-priority audio threads (`audio_thread_priority`) to ensure minimal latency.
   - The core Audio Render Thread works closely with `cpal`'s hardware callbacks, avoiding any unnecessary intermediate execution overhead.
   - Integrates `dasp` internally for high-quality, high-performance real-time resampling and channel conversion.

2. **Static Configuration & Dispatch**
   - Unlike the Web Audio API, which allows the dynamic addition and removal of nodes at runtime, this project uses **Static Graph Construction**.
   - All audio nodes are constructed and connected once via a `GraphBuilder` before audio processing starts.
   - Enforces the use of Enums (`NodeType`) to achieve Static Dispatch, completely avoiding the dynamic dispatch overhead of `Box<dyn Trait>` and allowing the compiler maximum optimization opportunities (e.g., Inlining).

3. **Zero Dynamic Allocation in Render Thread**
   - **Dynamic memory allocation and locks are strictly forbidden within the Audio Thread.** All necessary memory allocations are completed upfront during the `AudioContext` initialization phase.
   - Data exchange between nodes and buffer management rely entirely on pre-allocated fixed-size arrays or lock-free ring buffers (`ringbuf`).
   - Parameter updates and cross-thread communication (e.g., adjusting volume from the UI during playback) use lock-free mechanisms (`crossbeam-channel` or `Atomic` variables) to guarantee wait-free execution.

## 🧩 Built-in Audio Nodes

The library provides various basic audio nodes, categorized into Source Nodes (active signal generators) and Processing Nodes (passive effects):

### Source Nodes
- **`FileNode`**: Supports reading and playing various audio formats (MP3, WAV, etc., powered by `symphonia` and `hound`).
- **`MicrophoneNode`**: Captures real-time audio input from the hardware microphone.
- **`OscillatorNode`**: Digital signal generator (generates sine waves, square waves, sawtooth waves, etc., for synthesis).

### Processing & Effects Nodes
- **`GainNode`**: Gain (volume) control.
- **`MixerNode`**: Mixes audio signals from multiple input nodes into a single stream.
- **`DelayNode`**: Delay effect; can be combined with feedback loops to create echoes.
- **`FilterNode`**: Biquad filter supporting `HighPass`, `LowPass`, and `BandPass` to attenuate unwanted frequencies.
- **`ConvolverNode`**: Convolution reverb node capable of loading real-world IR (Impulse Response) files to achieve realistic spatial reverberations. Uses zero-latency partitioned convolution and worker threads for asynchronous computation to ensure high quality and prevent audio dropouts.

---

## 💻 Examples & Usage

Instead of embedding code directly, we provide several standalone runnable examples demonstrating how to use the API to build different audio topologies. You can find them in the `examples/` directory

### Running the Examples

You can run any of the examples using Cargo. For the best performance (to avoid audio glitches from the Convolver), it is highly recommended to run them in `release` mode:

```bash
cargo run --release --example play_karaoke
```
