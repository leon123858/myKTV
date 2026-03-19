import { useState } from "react";
import { ConfigProvider, Tabs, theme, Typography, Space } from "antd";
import {
  CustomerServiceOutlined,
  DownloadOutlined,
  PlayCircleOutlined,
} from "@ant-design/icons";
import { Downloader, Song } from "./components/Downloader";
import { KtvPlayer } from "./components/KtvPlayer";
import { Library } from "./components/Library";
import "./App.css";

const { Title, Text } = Typography;

function App() {
  const [activeTab, setActiveTab] = useState<string>("library");
  const [selectedSong, setSelectedSong] = useState<Song | null>(null);

  const handleSongSelected = (song: Song) => {
    setSelectedSong(song);
    setActiveTab("player");
  };

  const tabItems = [
    {
      key: "library",
      label: (
        <span>
          <CustomerServiceOutlined /> 曲庫
        </span>
      ),
      children: <Library onSongSelected={handleSongSelected} />,
    },
    {
      key: "downloader",
      label: (
        <span>
          <DownloadOutlined /> 下載
        </span>
      ),
      children: <Downloader />,
    },
    ...(selectedSong
      ? [
          {
            key: "player",
            label: (
              <span>
                <PlayCircleOutlined /> 播放
              </span>
            ),
            children: (
              <KtvPlayer
                song={selectedSong}
                onClose={() => setActiveTab("library")}
              />
            ),
          },
        ]
      : []),
  ];

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
        token: {
          colorPrimary: "#ff007f",
          colorBgContainer: "rgba(255, 255, 255, 0.04)",
          colorBgElevated: "rgba(30, 20, 50, 0.95)",
          borderRadius: 12,
          fontFamily: "'Outfit', sans-serif",
          fontSize: 14,
          colorLink: "#ff3399",
        },
        components: {
          Tabs: {
            inkBarColor: "#ff007f",
            itemActiveColor: "#ff3399",
            itemSelectedColor: "#ff3399",
            itemHoverColor: "#ff007f",
            cardBg: "transparent",
          },
          Button: {
            borderRadius: 10,
          },
          Input: {
            colorBgContainer: "rgba(255, 255, 255, 0.06)",
            colorBorder: "rgba(255, 255, 255, 0.15)",
          },
          Progress: {
            defaultColor: "#ff007f",
          },
          List: {
            colorBgContainer: "transparent",
          },
        },
      }}
    >
      <div className="orb orb-1" />
      <div className="orb orb-2" />
      <main className="app-container">
        <header className="header">
          <Title
            level={1}
            style={{
              margin: 0,
              fontSize: "2.4rem",
              fontWeight: 800,
              background: "linear-gradient(135deg, #ff007f, #6e00ff)",
              WebkitBackgroundClip: "text",
              WebkitTextFillColor: "transparent",
              backgroundClip: "text",
            }}
          >
            🎤 MyKTV
          </Title>
          <Text type="secondary" style={{ fontSize: "0.85rem" }}>
            powered by rust audio api
          </Text>
        </header>

        <Space direction="vertical" size={0} style={{ flex: 1, minHeight: 0 }}>
          <Tabs
            activeKey={activeTab}
            onChange={setActiveTab}
            items={tabItems}
            centered
            size="large"
            style={{ flex: 1 }}
            tabBarStyle={{
              marginBottom: 0,
            }}
          />
        </Space>
      </main>
    </ConfigProvider>
  );
}

export default App;
