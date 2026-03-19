import React, {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";

export interface Song {
    name: string;
    video_path: string;
    audio_path: string;
}

interface DownloaderProps {
}

export const Downloader: React.FC<DownloaderProps> = ({}) => {
    const [url, setUrl] = useState("");
    const [downloading, setDownloading] = useState(false);
    const [progress, setProgress] = useState<number>(0);
    const [statusText, setStatusText] = useState("");

    useEffect(() => {
        const unlisten = listen<string>("download-progress", (event) => {
            try {
                const data = JSON.parse(event.payload);
                if (data.status === "downloading") {
                    setProgress(data.percent || 0);
                    setStatusText(`Downloading: ${data.percent}%`);
                } else if (data.status === "finished") {
                    setStatusText("Download finished! Processing...");
                } else if (data.status === "completed") {
                    setDownloading(false);
                    setProgress(0);
                    setStatusText("Completed!");
                } else if (data.status === "starting") {
                    setStatusText("Starting download...");
                } else if (data.status === "error") {
                    setStatusText(`Error: ${data.message}`);
                    setDownloading(false);
                }
            } catch (e) {
                // ignore
            }
        });

        return () => {
            unlisten.then((fn) => fn());
        };
    }, []);

    const handleDownload = async () => {
        if (!url) return;
        setDownloading(true);
        setProgress(0);
        setStatusText("Initializing...");
        try {
            await invoke("download_youtube", {url});
        } catch (e) {
            console.error(e);
            setDownloading(false);
            setStatusText(`Error: ${e}`);
        }
    };

    return (
        <div className="downloader-container">
            <h2>Add New Song</h2>
            <div className="input-group">
                <input
                    type="text"
                    placeholder="Paste YouTube URL here..."
                    value={url}
                    disabled={downloading}
                    onChange={(e) => setUrl(e.target.value)}
                />
                <button className="btn" onClick={handleDownload} disabled={downloading || !url}>
                    {downloading ? "Downloading..." : "Download"}
                </button>
            </div>

            {downloading && (
                <div className="progress-container">
                    <div className="progress-bar" style={{width: `${progress}%`}}></div>
                    <p className="progress-text">{statusText}</p>
                </div>
            )}
        </div>
    );
};
