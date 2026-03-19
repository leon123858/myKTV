import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  List,
  Button,
  Empty,
  Typography,
  Space,
  Tooltip,
  Tag,
  Flex,
  Spin,
  Input,
} from "antd";
import {
  PlayCircleOutlined,
  FolderOpenOutlined,
  ReloadOutlined,
  CustomerServiceOutlined,
  AudioOutlined,
  VideoCameraOutlined,
  SearchOutlined,
} from "@ant-design/icons";

const { Title, Text } = Typography;

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

export const Library: React.FC<LibraryProps> = ({ onSongSelected }) => {
  const [songs, setSongs] = useState<Song[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchText, setSearchText] = useState("");

  const fetchSongs = async () => {
    setLoading(true);
    try {
      const resp = await invoke<Song[]>("get_downloaded_songs");
      setSongs(resp);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchSongs().catch(console.error);
  }, []);

  const filteredSongs = useMemo(() => {
    if (!searchText.trim()) return songs;
    const keyword = searchText.trim().toLowerCase();
    return songs.filter((s) => s.name.toLowerCase().includes(keyword));
  }, [songs, searchText]);

  return (
    <div className="panel-content">
      <Flex justify="space-between" align="center">
        <Title level={4} style={{ margin: 0 }}>
          <CustomerServiceOutlined style={{ marginRight: 8 }} />
          Library
          {songs.length > 0 && (
            <Tag
              color="magenta"
              style={{ marginLeft: 10, verticalAlign: "middle" }}
            >
              {songs.length} songs
            </Tag>
          )}
        </Title>
        <Space>
          <Tooltip title="Refresh">
            <Button
              icon={<ReloadOutlined />}
              onClick={fetchSongs}
              loading={loading}
              shape="circle"
              size="small"
            />
          </Tooltip>
          <Tooltip title="Open Folder">
            <Button
              icon={<FolderOpenOutlined />}
              onClick={handleOpenFolder}
              shape="circle"
              size="small"
            />
          </Tooltip>
        </Space>
      </Flex>

      {/* Search */}
      <Input
        placeholder="Search songs..."
        prefix={<SearchOutlined style={{ color: "rgba(255,255,255,0.3)" }} />}
        value={searchText}
        onChange={(e) => setSearchText(e.target.value)}
        allowClear
        size="middle"
      />

      {/* Song List — fixed height, scrollable */}
      <div className="library-list-container">
        <Spin spinning={loading}>
          {filteredSongs.length === 0 && !loading ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                <Text type="secondary">
                  {searchText
                    ? "No matching songs found"
: "No songs yet. Go to the \"Download\" tab to add some!"}
                </Text>
              }
              style={{ marginTop: 40 }}
            />
          ) : (
            <List
              pagination={{
                pageSize: 5,
                align: "center",
              }}
              dataSource={filteredSongs}
              split={false}
              renderItem={(song, index) => (
                <List.Item
                  key={index}
                  className="song-list-item"
                  style={{
                    background: "rgba(255,255,255,0.03)",
                    borderRadius: 10,
                    marginBottom: 8,
                    padding: "10px 16px",
                    border: "1px solid rgba(255,255,255,0.05)",
                    transition: "all 0.25s ease",
                  }}
                  actions={[
                    <Button
                      key="play"
                      type="primary"
                      icon={<PlayCircleOutlined />}
                      onClick={() => onSongSelected(song)}
                      style={{
                        background:
                          "linear-gradient(135deg, #00c6ff, #0072ff)",
                        border: "none",
                      }}
                      size="small"
                    >
                      Load
                    </Button>,
                  ]}
                >
                  <List.Item.Meta
                    avatar={
                      <div
                        style={{
                          width: 36,
                          height: 36,
                          borderRadius: 8,
                          background:
                            "linear-gradient(135deg, #2a004f, #6e00ff)",
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "center",
                          fontSize: 16,
                          boxShadow: "0 2px 8px rgba(110,0,255,0.3)",
                        }}
                      >
                        🎵
                      </div>
                    }
                    title={
                      <Text
                        ellipsis
                        style={{
                          color: "#fff",
                          fontSize: "0.9rem",
                          maxWidth: 500,
                        }}
                      >
                        {song.name}
                      </Text>
                    }
                    description={
                      <Space size={4}>
                        {song.video_path && (
                          <Tag
                            icon={<VideoCameraOutlined />}
                            color="blue"
                            style={{ fontSize: "0.7rem" }}
                          >
                            MV
                          </Tag>
                        )}
                        {song.audio_path && (
                          <Tag
                            icon={<AudioOutlined />}
                            color="purple"
                            style={{ fontSize: "0.7rem" }}
                          >
                            Audio
                          </Tag>
                        )}
                      </Space>
                    }
                  />
                </List.Item>
              )}
            />
          )}
        </Spin>
      </div>
    </div>
  );
};
