# 🎤 my-ktv

A free, fully functional desktop KTV system built with [Tauri 2](https://tauri.app/) + React + [rust-audio-api](https://github.com/leon123858/rust-audio-api).

Sing karaoke at home with just a microphone — no hardware mixer needed. All audio mixing is done in software with ultra-low latency.

## 📦 Download

Pre-built installers for Windows and macOS are available on the [**Releases**](https://github.com/leon123858/myKTV/releases/latest) page.

## ✨ Features

- 🎙️ **Real-time mixing** — Dry voice + Echo + Reverb mixed and output in real time
- 🔇 **Anti-feedback filters** — High-pass / low-pass filters automatically suppress howling noise
- 📥 **YouTube download** — Paste a YouTube URL to download songs; audio (MP3) and video (MP4) are automatically separated
- 🎬 **Synchronized MV playback** — Watch the music video while singing
- 🗂️ **Song library** — Downloaded songs are organized automatically; select and sing with one click
- 📁 **Local file import** — Import local audio files for playback

## 🧩 Architecture

### Audio Graph

```
Mic → HighPass(200Hz) → LowPass → Gain ─┬─ Dry Path ──────────────┐
                                          ├─ Delay → Echo Gain ─────┤
                                          └─ Convolver → LP → Reverb┤
                                                                     ▼
Music File → LowPass → Music Gain ──────────────────────────────→ Mixer → Output
```

### Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19 + TypeScript + Ant Design 6 |
| Desktop Runtime | Tauri 2 (Rust) |
| Audio Engine | [rust-audio-api](https://github.com/leon123858/rust-audio-api) (crates.io) |
| YouTube Downloader | Python sidecar (yt-dlp + FFmpeg) |
| Build Tool | Vite 7 |

## 📋 Prerequisites

- [Node.js](https://nodejs.org/) ≥ 18
- [Yarn](https://yarnpkg.com/) package manager
- [Rust](https://www.rust-lang.org/tools/install) toolchain
- [FFmpeg](https://ffmpeg.org/) — **must be installed and available in PATH** (required for YouTube downloads)
- [Python 3](https://www.python.org/) + [uv](https://github.com/astral-sh/uv) — needed to build the downloader sidecar

## 🚀 Quick Start

### 1. Install frontend dependencies

```bash
cd my-ktv
yarn install
```

### 2. Build the Python Downloader sidecar

```bash
# Windows
yarn build:sidecar:win
```

This command packages `python-downloader/downloader.py` into a standalone executable and copies it to the `src-tauri/` directory.

### 3. Run in development mode

```bash
yarn tauri dev
```

### 4. Build for production

```bash
yarn tauri build
```

## 🎮 Usage

1. **Download songs** — Switch to the `Download` tab, paste a YouTube URL, and wait for the download and audio/video separation to complete
2. **Browse library** — Go to the `Library` tab to see all downloaded songs
3. **Start singing** — Select a song to enter the `Player` tab, then press **Start KTV** to begin

## 📁 Directory Structure

```
my-ktv/
├── src/                    # React frontend source
│   ├── components/
│   │   ├── Downloader.tsx  # YouTube downloader component
│   │   ├── KtvPlayer.tsx   # KTV player component
│   │   └── Library.tsx     # Song library component
│   └── App.tsx             # Main app component (tab navigation)
├── src-tauri/              # Tauri / Rust backend
│   └── src/lib.rs          # Audio engine, graph builder, Tauri commands
├── python-downloader/      # Python downloader (yt-dlp sidecar)
│   └── downloader.py       # YouTube download + FFmpeg audio/video split
├── package.json
└── vite.config.ts
```

## 📄 License

[Apache License 2.0](../LICENSE)
