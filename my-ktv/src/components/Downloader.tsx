import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Input,
  Button,
  Progress,
  Steps,
  Alert,
  Space,
  Typography,
  Card,
  Flex,
} from "antd";
import {
  DownloadOutlined,
  LoadingOutlined,
  CheckCircleOutlined,
  CloudDownloadOutlined,
  ScissorOutlined,
  SmileOutlined,
  LinkOutlined,
} from "@ant-design/icons";

const { Title, Text } = Typography;

export interface Song {
  name: string;
  video_path: string;
  audio_path: string;
}

type DownloadStage =
  | "idle"
  | "starting"
  | "downloading"
  | "finished"
  | "processing"
  | "completed"
  | "error";

const stageToStep: Record<DownloadStage, number> = {
  idle: -1,
  starting: 0,
  downloading: 1,
  finished: 1,
  processing: 2,
  completed: 3,
  error: -1,
};

export const Downloader: React.FC = () => {
  const [url, setUrl] = useState("");
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<number>(0);
  const [statusText, setStatusText] = useState("");
  const [stage, setStage] = useState<DownloadStage>("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const [fileName, setFileName] = useState("");

  useEffect(() => {
    const unlisten = listen<string>("download-progress", (event) => {
      try {
        const data = JSON.parse(event.payload);
        // console.log("Downloader:", data);
        if (data.status === "downloading") {
          setProgress(data.percent || 0);
          setStatusText(`下載中: ${data.percent?.toFixed(1)}%`);
          setStage("downloading");
          if (data.filename) {
            const name = data.filename.split(/[\\/]/).pop() || "";
            setFileName(name);
          }
        } else if (data.status === "finished") {
          setStatusText("檔案下載完成，準備處理...");
          setProgress(100);
          setStage("finished");
        } else if (data.status === "processing") {
          setStatusText("分離音訊與影片中...");
          setStage("processing");
        } else if (data.status === "completed") {
          setDownloading(false);
          setProgress(100);
          setStatusText("完成！");
          setStage("completed");
        } else if (data.status === "starting") {
          setStatusText("啟動下載器...");
          setStage("starting");
        } else if (data.status === "error") {
          setStatusText(`錯誤: ${data.message}`);
          setErrorMsg(data.message || "Unknown error");
          setDownloading(false);
          setStage("error");
        }
      } catch {
        // ignore parse errors
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
    setStatusText("初始化中...");
    setStage("starting");
    setErrorMsg("");
    setFileName("");
    try {
      await invoke("download_youtube", { url });
    } catch (e) {
      console.error(e);
      setDownloading(false);
      setErrorMsg(`${e}`);
      setStage("error");
    }
  };

  const handleReset = () => {
    setUrl("");
    setStage("idle");
    setProgress(0);
    setStatusText("");
    setErrorMsg("");
    setFileName("");
  };

  const currentStep = stageToStep[stage];

  const stepItems = [
    {
      title: "啟動",
      icon:
        stage === "starting" ? <LoadingOutlined /> : <CloudDownloadOutlined />,
    },
    {
      title: "下載",
      icon:
        stage === "downloading" ? (
          <LoadingOutlined />
        ) : stage === "finished" ? (
          <CheckCircleOutlined />
        ) : (
          <DownloadOutlined />
        ),
      description:
        stage === "downloading" ? `${progress.toFixed(1)}%` : undefined,
    },
    {
      title: "處理",
      icon:
        stage === "processing" ? <LoadingOutlined /> : <ScissorOutlined />,
    },
    {
      title: "完成",
      icon: <SmileOutlined />,
    },
  ];

  return (
    <div className="panel-content">
      <Title level={4} style={{ margin: 0 }}>
        <DownloadOutlined style={{ marginRight: 8 }} />
        新增歌曲
      </Title>

      <Card
        style={{
          background: "rgba(0,0,0,0.2)",
          border: "1px solid rgba(255,255,255,0.06)",
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Flex gap={10}>
          <Input
            placeholder="貼上 YouTube 連結..."
            prefix={
              <LinkOutlined style={{ color: "rgba(255,255,255,0.3)" }} />
            }
            value={url}
            disabled={downloading}
            onChange={(e) => setUrl(e.target.value)}
            onPressEnter={handleDownload}
            size="large"
            allowClear
          />
          <Button
            type="primary"
            size="large"
            icon={<DownloadOutlined />}
            onClick={handleDownload}
            loading={downloading}
            disabled={downloading || !url}
            style={{
              background: downloading
                ? undefined
                : "linear-gradient(135deg, #ff007f, #6e00ff)",
              border: "none",
              minWidth: 100,
            }}
          >
            {downloading ? "下載中" : "下載"}
          </Button>
        </Flex>
      </Card>

      {stage !== "idle" && (
        <Card
          style={{
            background: "rgba(0,0,0,0.15)",
            border: "1px solid rgba(255,255,255,0.06)",
          }}
          styles={{ body: { padding: "16px 20px" } }}
        >
          <Space direction="vertical" size="middle" style={{ width: "100%" }}>
            <Steps
              current={currentStep}
              size="small"
              items={stepItems}
              status={stage === "error" ? "error" : undefined}
            />

            {stage === "downloading" && (
              <Progress
                percent={Math.round(progress)}
                strokeColor={{
                  "0%": "#ff007f",
                  "100%": "#6e00ff",
                }}
                trailColor="rgba(255,255,255,0.08)"
                size={["100%", 14]}
                format={(p) => `${p}%`}
              />
            )}

            {(stage === "starting" ||
              stage === "finished" ||
              stage === "processing") && (
                <Progress
                  percent={100}
                  strokeColor={{
                    "0%": "#ff007f",
                    "100%": "#6e00ff",
                  }}
                  trailColor="rgba(255,255,255,0.08)"
                  size={["100%", 14]}
                  status="active"
                  format={() => statusText}
                />
              )}

            {fileName && (
              <Text
                type="secondary"
                ellipsis
                style={{ fontSize: "0.8rem", display: "block" }}
              >
                📁 {fileName}
              </Text>
            )}

            {stage === "completed" && (
              <Alert
                type="success"
                message="下載完成！歌曲已加入曲庫。"
                showIcon
                icon={<CheckCircleOutlined />}
                action={
                  <Button size="small" onClick={handleReset}>
                    再下載一首
                  </Button>
                }
              />
            )}

            {stage === "error" && (
              <Alert
                type="error"
                message="下載失敗"
                description={errorMsg}
                showIcon
                action={
                  <Button size="small" danger onClick={handleReset}>
                    重試
                  </Button>
                }
              />
            )}
          </Space>
        </Card>
      )}
    </div>
  );
};
