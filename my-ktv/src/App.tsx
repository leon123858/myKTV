import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [currentFile, setCurrentFile] = useState<string>("No file selected");
  const [statusMsg, setStatusMsg] = useState<string>("");
  const [isPlaying, setIsPlaying] = useState<boolean>(false);

  async function handleUpload() {
    try {
      const filePath = await invoke<string>("upload_audio_file");
      setCurrentFile(filePath);
      setStatusMsg(`File selected: ${filePath.split(/[/\\]/).pop()}`);
    } catch (error) {
      setStatusMsg(`Upload failed: ${error}`);
    }
  }

  async function handlePlay() {
    if (currentFile === "No file selected") {
      setStatusMsg("Please select a file first!");
      return;
    }

    try {
      const result = await invoke<string>("play_audio_file", { path: currentFile });
      setStatusMsg(result);
      setIsPlaying(true);
    } catch (error) {
      setStatusMsg(`Play failed: ${error}`);
      setIsPlaying(false);
    }
  }

  async function handleStop() {
    try {
      const result = await invoke<string>("stop_audio");
      setStatusMsg(result);
      setIsPlaying(false);
    } catch (error) {
      setStatusMsg(`Stop failed: ${error}`);
    }
  }

  return (
    <main className="container">
      <h1>My KTV - Audio Player</h1>

      <div className="card">
        <h2>🎵 File Controls</h2>

        <div className="file-info">
          <p><strong>Selected File:</strong></p>
          <p className="file-path">{currentFile.split(/[/\\]/).pop() || "None"}</p>
        </div>

        <div className="button-group">
          <button onClick={handleUpload} className="btn-upload">
            📁 Upload Audio File
          </button>

          <button
            onClick={handlePlay}
            disabled={currentFile === "No file selected" || isPlaying}
            className="btn-play"
          >
            ▶️ Play
          </button>

          <button
            onClick={handleStop}
            disabled={!isPlaying}
            className="btn-stop"
          >
            ⏹️ Stop
          </button>
        </div>

        {statusMsg && (
          <div className={`status-message ${statusMsg.includes("failed") ? "error" : "success"}`}>
            {statusMsg}
          </div>
        )}
      </div>

      <div className="info">
        <p>Supported formats: MP3, WAV, FLAC, OGG, M4A</p>
        <p>Status: {isPlaying ? "🔊 Playing" : "⏸️ Stopped"}</p>
      </div>
    </main>
  );
}

export default App;
