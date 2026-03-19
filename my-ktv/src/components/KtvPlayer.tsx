import React, {useEffect, useRef, useState} from "react";
import {convertFileSrc, invoke} from "@tauri-apps/api/core";
import {Song} from "./Downloader";

interface KtvPlayerProps {
    song: Song | null;
    onClose: () => void;
}

export const KtvPlayer: React.FC<KtvPlayerProps> = ({song, onClose}) => {

    const videoRef = useRef<HTMLVideoElement>(null);
    const [isPlaying, setIsPlaying] = useState(false);
    const [errorMsg, setErrorMsg] = useState("");

    const handleStart = async () => {
        if (!song) return;
        try {
            await invoke("start_karaoke", {path: song.audio_path});
            if (videoRef.current) {
                videoRef.current.play();
            }
            setIsPlaying(true);
            setErrorMsg("");
        } catch (e) {
            console.error(e);
            setErrorMsg(`Error starting karaoke: ${e}`);
        }
    };

    const handleStop = async () => {
        try {
            await invoke("stop_karaoke");
            if (videoRef.current) {
                videoRef.current.pause();
                videoRef.current.currentTime = 0;
            }
            setIsPlaying(false);
        } catch (e) {
            console.error(e);
            setErrorMsg(`Error stopping karaoke: ${e}`);
        }
    };

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            invoke("stop_karaoke").catch(console.error);
        };
    }, []);

    return (
        <div className="ktv-player">
            <div className="ktv-header">
                <h2>Now Playing: {song?.name || "None"}</h2>
                <button className="btn btn-close" onClick={onClose}>
                    Exit
                </button>
            </div>

            <div className="video-container">
                {song ? (
                    <video
                        ref={videoRef}
                        src={convertFileSrc(song.video_path)}
                        muted
                        loop
                        className="bg-video"
                    />
                ) : (
                    <div className="no-video">Please select a song first.</div>
                )}
            </div>

            {errorMsg && <div className="error-msg">{errorMsg}</div>}

            <div className="player-controls">
                {!isPlaying ? (
                    <button className="btn btn-play" onClick={handleStart} disabled={!song}>
                        ▶ Start KTV
                    </button>
                ) : (
                    <button className="btn btn-stop" onClick={handleStop}>
                        ■ Stop KTV
                    </button>
                )}
            </div>
        </div>
    );
};
