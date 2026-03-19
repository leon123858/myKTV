import React, { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Song } from "./Downloader";
import {
  Button,
  Typography,
  Alert,
  Space,
  Tag,
  Flex,
  Tooltip,
} from "antd";
import {
  CaretRightOutlined,
  PauseOutlined,
  ArrowLeftOutlined,
  SoundOutlined,
} from "@ant-design/icons";

const { Title, Text } = Typography;

interface KtvPlayerProps {
  song: Song | null;
  onClose: () => void;
}

export const KtvPlayer: React.FC<KtvPlayerProps> = ({ song, onClose }) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");

  const handleStart = async () => {
    if (!song) return;
    try {
      await invoke("start_karaoke", { path: song.audio_path });
      if (videoRef.current) {
        videoRef.current.play();
      }
      setIsPlaying(true);
      setErrorMsg("");
    } catch (e) {
      console.error(e);
      setErrorMsg(`啟動 KTV 時發生錯誤: ${e}`);
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
      setErrorMsg(`停止 KTV 時發生錯誤: ${e}`);
    }
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      invoke("stop_karaoke").catch(console.error);
    };
  }, []);

  return (
    <div className="panel-content">
      {/* Header */}
      <Flex justify="space-between" align="center">
        <Space>
          <Tooltip title="返回曲庫">
            <Button
              icon={<ArrowLeftOutlined />}
              onClick={onClose}
              shape="circle"
              size="small"
            />
          </Tooltip>
          <Title
            level={4}
            ellipsis
            style={{ margin: 0, maxWidth: 600 }}
          >
            <SoundOutlined style={{ marginRight: 8 }} />
            {song?.name || "未選擇歌曲"}
          </Title>
        </Space>
        {isPlaying && (
          <Tag color="#ff007f" className="pulse-tag">
            🎤 LIVE
          </Tag>
        )}
      </Flex>

      {/* Video */}
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
          <div className="no-video">
            <Text type="secondary" style={{ fontSize: "1.1rem" }}>
              請先選擇一首歌曲
            </Text>
          </div>
        )}
        {isPlaying && <div className="video-overlay-glow" />}
      </div>

      {/* Error */}
      {errorMsg && (
        <Alert
          type="error"
          message={errorMsg}
          showIcon
          closable
          onClose={() => setErrorMsg("")}
          style={{ borderRadius: 10 }}
        />
      )}

      {/* Controls */}
      <Flex justify="center" gap={16}>
        {!isPlaying ? (
          <Button
            type="primary"
            size="large"
            icon={<CaretRightOutlined />}
            onClick={handleStart}
            disabled={!song}
            style={{
              background: "linear-gradient(135deg, #ff007f, #6e00ff)",
              border: "none",
              height: 48,
              paddingInline: 40,
              fontSize: "1rem",
              fontWeight: 600,
              boxShadow: "0 4px 20px rgba(255,0,127,0.35)",
            }}
          >
            開始 KTV
          </Button>
        ) : (
          <Button
            danger
            size="large"
            icon={<PauseOutlined />}
            onClick={handleStop}
            style={{
              height: 48,
              paddingInline: 40,
              fontSize: "1rem",
              fontWeight: 600,
            }}
          >
            停止 KTV
          </Button>
        )}
      </Flex>
    </div>
  );
};
