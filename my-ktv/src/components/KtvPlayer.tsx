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
        <Space>
          {isPlaying && (
            <>
              <Tag color="#ff007f" className="pulse-tag">
                🎤 LIVE
              </Tag>
              <Button
                danger
                size="small"
                icon={<PauseOutlined />}
                onClick={handleStop}
                style={{
                  fontWeight: 600,
                }}
              >
                停止 KTV
              </Button>
            </>
          )}
        </Space>
      </Flex>

      {/* Video */}
      <div className="video-container" style={{ position: "relative" }}>
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
        {/* Start button overlay centered on video */}
        {!isPlaying && song && (
          <div
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: "100%",
              height: "100%",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 10,
            }}
          >
            <Button
              type="primary"
              size="large"
              icon={<CaretRightOutlined />}
              onClick={handleStart}
              style={{
                background: "linear-gradient(135deg, #ff007f, #6e00ff)",
                border: "none",
                height: 56,
                paddingInline: 48,
                fontSize: "1.1rem",
                fontWeight: 600,
                boxShadow: "0 4px 24px rgba(255,0,127,0.5)",
                borderRadius: 28,
              }}
            >
              開始 KTV
            </Button>
          </div>
        )}
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
    </div>
  );
};
