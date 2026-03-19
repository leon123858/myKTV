import React, {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";

export interface Song {
    name: string;
    video_path: string;
    audio_path: string;
}

interface LibraryProps {
    onSongSelected: (song: Song) => void;
}

async function handleOpenFolder() {
    try {
        await invoke("open_app_dir");
    } catch (error) {
        console.error("can not open lib folder:", error);
    }
}

export const Library: React.FC<LibraryProps> = ({onSongSelected}) => {
    const [songs, setSongs] = useState<Song[]>([]);

    const fetchSongs = async () => {
        try {
            const resp = await invoke<Song[]>("get_downloaded_songs");
            setSongs(resp);
        } catch (e) {
            console.error(e);
        }
    };

    useEffect(() => {
        fetchSongs().catch(err => {
            console.error(err);
        })
    }, []);

    return (
        <div className="downloader-container">
            <h2>Songs Library</h2>

            <div className="songs-list">
                <h3 style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    Available Songs
                    <button onClick={() => handleOpenFolder()}>{ 'folder' }</button>
                </h3>
                {songs.length === 0 ? (
                    <p>No songs downloaded yet.</p>
                ) : (
                    <ul>
                        {songs.map((song, i) => (
                            <li key={i} className="song-item">
                                <span className="song-title">{song.name}</span>
                                <button className="btn btn-play" onClick={() => onSongSelected(song)}>
                                    Load
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
            </div>
        </div>
    );
};
