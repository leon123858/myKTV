# 🎤 myKTV

**A free, open-source, home-use software-only KTV system.**

No hardware mixer needed — just a microphone and a computer to enjoy a full karaoke experience at home. Real-time audio mixing is powered by [rust-audio-api](https://github.com/leon123858/rust-audio-api), providing echo, reverb, and other vocal effects entirely in software.

## 📂 Projects

This repository contains two sub-projects:

| Project | Path | Description | Status |
|---------|------|-------------|--------|
| **[my-ktv](./my-ktv)** | `my-ktv/` | Desktop KTV application (Tauri + React) | ✅ Production-ready |
| **[web-ktv](./web-ktv)** | `web-ktv/` | Web Audio API demo (Next.js) | ⚠️ Demo only |

## 🎵 my-ktv — Desktop KTV

A desktop application built with Tauri 2. It performs real-time audio mixing at the system level via a native Rust audio engine, delivering **ultra-low latency** karaoke.

**Key Features:**
- 🎙️ Real-time microphone mixing (dry voice + echo + reverb)
- 📥 Download songs from YouTube (auto-splits audio & video)
- 🎬 Synchronized music video playback
- 🗂️ Local song library management

👉 **[Download the latest release](https://github.com/leon123858/myKTV/releases/latest)** (Windows / macOS) or build from source — see [my-ktv/README.md](./my-ktv/README.md) for details.

## 🌐 web-ktv — Web Demo

A Next.js application demonstrating a similar audio graph using the browser's Web Audio API. Due to inherent browser audio latency, **it is not suitable for actual singing** and serves only as a technical demonstration.

## ⚙️ Technical Highlights

- **Software-only mixing** — No hardware mixer required; real-time audio processing is handled by [rust-audio-api](https://github.com/leon123858/rust-audio-api)
- **Audio Graph architecture** — Mic → Filters → Gain → Echo / Reverb → Mixer → Output
- **Cross-platform** — Built on Tauri 2, supporting Windows / macOS / Linux

## 📄 License

This project is licensed under the [Apache License 2.0](./LICENSE).
