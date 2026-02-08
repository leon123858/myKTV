import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [currentFile, setCurrentFile] = useState<string>("No file selected");
  const [statusMsg, setStatusMsg] = useState<string>("");
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [isKaraoke, setIsKaraoke] = useState<boolean>(false);
  const [isMicOnly, setIsMicOnly] = useState<boolean>(false);

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

  async function handleStartMic() {
    try {
      const result = await invoke<string>("start_mic_only");
      setStatusMsg(result);
      setIsMicOnly(true);
    } catch (error) {
      setStatusMsg(`Mic start failed: ${error}`);
      setIsMicOnly(false);
    }
  }

  async function handleStopMic() {
    try {
      const result = await invoke<string>("stop_mic");
      setStatusMsg(result);
      setIsMicOnly(false);
    } catch (error) {
      setStatusMsg(`Mic stop failed: ${error}`);
    }
  }

  async function handleStartKaraoke() {
    if (currentFile === "No file selected") {
      setStatusMsg("Please select a music file first!");
      return;
    }

    try {
      const result = await invoke<string>("start_karaoke", { path: currentFile });
      setStatusMsg(result);
      setIsKaraoke(true);
    } catch (error) {
      setStatusMsg(`Karaoke start failed: ${error}`);
      setIsKaraoke(false);
    }
  }

  async function handleStopKaraoke() {
    try {
      const result = await invoke<string>("stop_karaoke");
      setStatusMsg(result);
      setIsKaraoke(false);
    } catch (error) {
      setStatusMsg(`Karaoke stop failed: ${error}`);
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
            disabled={currentFile === "No file selected" || isPlaying || isKaraoke}
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

      <div className="card karaoke-card">
        <h2>🎤 Karaoke Mode</h2>
        <p className="karaoke-info">
          Mix background music with microphone input for singing along!
        </p>

        <div className="button-group">
          <button
            onClick={handleStartKaraoke}
            disabled={currentFile === "No file selected" || isKaraoke || isPlaying}
            className="btn-karaoke-start"
          >
            🎤 Start Karaoke
          </button>

          <button
            onClick={handleStopKaraoke}
            disabled={!isKaraoke}
            className="btn-karaoke-stop"
          >
            ⏹️ Stop Karaoke
          </button>
        </div>

        <div className="karaoke-status">
          <p><strong>Music:</strong> {isKaraoke ? "🎵 Playing" : "⏸️ Stopped"}</p>
          <p><strong>Microphone:</strong> {isKaraoke ? "🎤 Active" : "🔇 Inactive"}</p>
        </div>
      </div>

      <div className="card mic-test-card">
        <h2>🎙️ Microphone Test</h2>
        <p className="mic-test-info">
          Test your microphone alone without background music
        </p>

        <div className="button-group">
          <button
            onClick={handleStartMic}
            disabled={isMicOnly || isKaraoke || isPlaying}
            className="btn-mic-start"
          >
            🎙️ Start Mic
          </button>

          <button
            onClick={handleStopMic}
            disabled={!isMicOnly}
            className="btn-mic-stop"
          >
            ⏹️ Stop Mic
          </button>
        </div>

        <div className="mic-test-status">
          <p><strong>Status:</strong> {isMicOnly ? "🎙️ Mic Active" : "🔇 Mic Inactive"}</p>
        </div>
      </div>

      <div className="info">
        <p>Supported formats: MP3, WAV, FLAC, OGG, M4A</p>
        <p>Status: {isPlaying ? "🔊 Playing" : isKaraoke ? "🎤 Karaoke Mode" : isMicOnly ? "🎙️ Mic Test" : "⏸️ Stopped"}</p>
      </div>
    </main>
  );
}

export default App;
