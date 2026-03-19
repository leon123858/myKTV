import {useState} from "react";
import {Downloader, Song} from "./components/Downloader";
import {KtvPlayer} from "./components/KtvPlayer";
import "./App.css";
import {Library} from "./components/Library.tsx";

function App() {
    const [activeTab, setActiveTab] = useState<"library" | "downloader" | "player">("library");
    const [selectedSong, setSelectedSong] = useState<Song | null>(null);

    const handleSongSelected = (song: Song) => {
        setSelectedSong(song);
        setActiveTab("player");
    };

    return (
        <>
            <div className="orb orb-1"></div>
            <div className="orb orb-2"></div>
            <main className="app-container">
                <header className="header">
                    <h1>MyKTV</h1>
                    <p>powered by rust audio api</p>
                </header>

                <nav className="tabs">
                    <button
                        className={`tab-btn ${activeTab === 'library' ? 'active' : ''}`}
                        onClick={() => setActiveTab('library')}
                    >
                        Library
                    </button>
                    <button
                        className={`tab-btn ${activeTab === 'downloader' ? 'active' : ''}`}
                        onClick={() => setActiveTab('downloader')}
                    >
                        Downloader
                    </button>
                    {selectedSong && (
                        <button
                            className={`tab-btn ${activeTab === 'player' ? 'active' : ''}`}
                            onClick={() => setActiveTab('player')}
                        >
                            Player
                        </button>
                    )}
                </nav>

                <div className="content-area">
                    {activeTab === "library" && (
                        <div className="library-view">
                            <Library onSongSelected={handleSongSelected}/>
                        </div>
                    )}

                    {activeTab === "downloader" && (
                        <div className="downloader-view">
                            <Downloader/>
                        </div>
                    )}

                    {activeTab === "player" && (
                        <KtvPlayer
                            song={selectedSong}
                            onClose={() => setActiveTab("library")}
                        />
                    )}
                </div>
            </main>
        </>
    );
}

export default App;
