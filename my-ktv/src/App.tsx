import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Mode = "idle" | "playing_music" | "mic_only" | "karaoke";

function App() {
  const [currentFile, setCurrentFile] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("idle");
  const [statusMsg, setStatusMsg] = useState<string>("Ready to rock");

  useEffect(() => {
    // Check if there's an already loaded file on init
    invoke<string>("get_current_file")
      .then((file) => {
        if (file && file !== "No file loaded") {
          setCurrentFile(file);
        }
      })
      .catch(console.error);
  }, []);

  const handleUpload = async () => {
    try {
      const path = await invoke<string>("upload_audio_file");
      if (path) {
        setCurrentFile(path);
        setStatusMsg("File loaded successfully");
      }
    } catch (err) {
      console.error(err);
      setStatusMsg("Upload canceled or failed");
    }
  };

  const handlePlayMusic = async () => {
    if (!currentFile) {
      alert("Please upload an audio file first!");
      return;
    }
    
    // Stop any current activity
    if (mode === "mic_only") await invoke("stop_mic");
    if (mode === "karaoke") await invoke("stop_karaoke");
    
    try {
      await invoke("play_audio_file", { path: currentFile });
      setMode("playing_music");
      setStatusMsg("Playing Music");
    } catch (err) {
      console.error(err);
      setStatusMsg(`Error: ${err}`);
    }
  };

  const handleStopMusic = async () => {
    try {
      await invoke("stop_audio");
      setMode("idle");
      setStatusMsg("Music Stopped");
    } catch (err) {
      console.error(err);
    }
  };

  const handleStartMic = async () => {
    // Stop any current activity
    if (mode === "playing_music") await invoke("stop_audio");
    if (mode === "karaoke") await invoke("stop_karaoke");

    try {
      await invoke("start_mic_only");
      setMode("mic_only");
      setStatusMsg("Microphone Active");
    } catch (err) {
      console.error(err);
      setStatusMsg(`Error: ${err}`);
    }
  };

  const handleStopMic = async () => {
    try {
      await invoke("stop_mic");
      setMode("idle");
      setStatusMsg("Microphone Stopped");
    } catch (err) {
      console.error(err);
    }
  };

  const handleStartKaraoke = async () => {
    if (!currentFile) {
      alert("Please upload a backing track first!");
      return;
    }

    // Stop any current activity
    if (mode === "playing_music") await invoke("stop_audio");
    if (mode === "mic_only") await invoke("stop_mic");

    try {
      await invoke("start_karaoke", { path: currentFile });
      setMode("karaoke");
      setStatusMsg("Karaoke Mode ON 🔥");
    } catch (err) {
      console.error(err);
      setStatusMsg(`Error: ${err}`);
    }
  };

  const handleStopKaraoke = async () => {
    try {
      await invoke("stop_karaoke");
      setMode("idle");
      setStatusMsg("Karaoke Stopped");
    } catch (err) {
      console.error(err);
    }
  };

  // Helper to extract filename from path
  const getFilename = (path: string) => {
    return path.split(/[/\\]/).pop();
  };

  return (
    <>
      <div className="orb orb-1"></div>
      <div className="orb orb-2"></div>
      <main className="app-container">
        <header className="header">
          <h1>MyKTV Studio</h1>
          <p>Next-Gen Rust Audio Experience</p>
        </header>

        <section className="track-container">
          <div className="track-info">
            <div className="track-icon">🎵</div>
            <div className="track-details">
              <h3>Target Track</h3>
              <p>{currentFile ? getFilename(currentFile) : "No File Selected"}</p>
            </div>
          </div>
          <button className="btn btn-upload" onClick={handleUpload}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
              <polyline points="17 8 12 3 7 8"></polyline>
              <line x1="12" y1="3" x2="12" y2="15"></line>
            </svg>
            Select Track
          </button>
        </section>

        <section className="controls-grid">
          {/* Music Control */}
          <div className="control-card">
            <h4>Music Only</h4>
            {mode === "playing_music" ? (
              <button className="btn btn-stop" onClick={handleStopMusic}>
                ■ Stop Music
              </button>
            ) : (
              <button className="btn btn-play" onClick={handlePlayMusic} disabled={!currentFile}>
                ▶ Play Music
              </button>
            )}
          </div>

          {/* Mic Control */}
          <div className="control-card">
            <h4>Mic Check</h4>
            {mode === "mic_only" ? (
              <button className="btn btn-stop" onClick={handleStopMic}>
                ■ Turn Off
              </button>
            ) : (
              <button className="btn btn-mic" onClick={handleStartMic}>
                🎙️ Start Mic
              </button>
            )}
          </div>

          {/* Karaoke Control */}
          <div className="control-card">
            <h4>Full KTV</h4>
            {mode === "karaoke" ? (
              <button className="btn btn-stop" onClick={handleStopKaraoke}>
                ■ End Session
              </button>
            ) : (
              <button className="btn btn-karaoke" onClick={handleStartKaraoke} disabled={!currentFile}>
                ✨ Start KTV
              </button>
            )}
          </div>
        </section>

        <div className={`status-bar ${mode !== "idle" ? "status-playing" : ""}`}>
          {mode !== "idle" && <span style={{ marginRight: "10px" }}>⚡</span>}
          {statusMsg}
        </div>
      </main>
    </>
  );
}

export default App;
