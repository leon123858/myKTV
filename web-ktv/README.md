# 🌐 web-ktv

A Web Audio API KTV demo built with [Next.js](https://nextjs.org/), demonstrating real-time audio mixing in the browser using JavaScript.

> ⚠️ **Note:** Due to inherent browser Web Audio API latency, this project **cannot be used for actual KTV singing**. It serves only as a technical demonstration and proof of concept. For a production-ready KTV experience, please use [my-ktv](../my-ktv).

## ✨ Demo Features

- 🎙️ **Microphone input** — Captures mic audio stream through the browser
- 🔊 **Real-time mixing** — Adjustable Mic Gain / Music Gain / Echo / Reverb
- 🎛️ **Dynamic compressor** — Tunable Threshold, Ratio, Knee, Attack, Release
- 🎵 **Local music playback** — Upload audio files as background music with play / pause / reset
- 📊 **Real-time visualizer** — Debug waveforms for MIC input and Master output

## 🧩 Audio Graph

```
Mic → HighPass(200Hz) → Presence EQ → micGain ─┬─ Dry Path ──────────────┐
                                                 ├─ Delay → Echo Feedback ─┤
                                                 └─ Convolver → reverbGain ┤
                                                                            ▼
Upload Audio → musicGain ─────────────────────────────────────────────→ Compressor → Output
```

## 📋 Prerequisites

- [Node.js](https://nodejs.org/) ≥ 18
- [Yarn](https://yarnpkg.com/) package manager
- A browser with microphone support (Chrome / Edge recommended)

## 🚀 Quick Start

```bash
# Install dependencies
yarn install

# Start the development server
yarn dev
```

Open [http://localhost:3000](http://localhost:3000) to see the home page, then navigate to `/ktv` to enter the KTV page.

## 🎮 Usage

1. Navigate to `/ktv`
2. Click the **Start Microphone Engine** button (the browser will request microphone permission)
3. Upload a local audio file as background music
4. Adjust Mic / Music / Echo / Reverb parameters
5. Enable Debug mode to view real-time audio waveforms

## 📁 Directory Structure

```
web-ktv/
├── app/
│   ├── page.tsx            # Home page
│   ├── ktv/
│   │   └── page.tsx        # KTV page (main functionality)
│   ├── components/
│   │   ├── Visualizer.tsx  # Audio waveform visualizer
│   │   └── controlSide.tsx # Control slider component
│   ├── libs/
│   │   ├── graph.ts        # Audio Graph construction & control
│   │   └── ir.ts           # Impulse Response (IR) generation
│   └── types/
│       └── types.ts        # TypeScript type definitions
├── package.json
└── next.config.ts
```

## 🛠️ Tech Stack

| Technology | Purpose |
|------------|---------|
| Next.js 16 | Framework |
| React 19 | UI |
| Tailwind CSS 4 | Styling |
| Web Audio API | Audio processing |
| Chart.js | Waveform visualization |

## 📄 License

[Apache License 2.0](../LICENSE)
