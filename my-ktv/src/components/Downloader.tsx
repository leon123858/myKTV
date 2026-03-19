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
          setStatusText(`Downloading: ${data.percent?.toFixed(1)}%`);
          setStage("downloading");
          if (data.filename) {
            const name = data.filename.split(/[\\/]/).pop() || "";
            setFileName(name);
          }
        } else if (data.status === "finished") {
          setStatusText("File download complete, preparing to process...");
          setProgress(100);
          setStage("finished");
        } else if (data.status === "processing") {
          setStatusText("Splitting audio and video...");
          setStage("processing");
        } else if (data.status === "completed") {
          setDownloading(false);
          setProgress(100);
          setStatusText("Completed!");
          setStage("completed");
        } else if (data.status === "starting") {
          setStatusText("Starting downloader...");
          setStage("starting");
        } else if (data.status === "error") {
          setStatusText(`Error: ${data.message}`);
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
    setStatusText("Initializing...");
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
      title: "Start",
      icon:
        stage === "starting" ? <LoadingOutlined /> : <CloudDownloadOutlined />,
    },
    {
      title: "Download",
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
      title: "Process",
      icon:
        stage === "processing" ? <LoadingOutlined /> : <ScissorOutlined />,
    },
    {
      title: "Complete",
      icon: <SmileOutlined />,
    },
  ];

  return (
    <div className="panel-content">
      <Title level={4} style={{ margin: 0 }}>
        <DownloadOutlined style={{ marginRight: 8 }} />
        Add Song
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
            placeholder="Paste YouTube link..."
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
            {downloading ? "Downloading" : "Download"}
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
                message="Download complete! Song added to library."
                showIcon
                icon={<CheckCircleOutlined />}
                action={
                  <Button size="small" onClick={handleReset}>
                    Download another
                  </Button>
                }
              />
            )}

            {stage === "error" && (
              <Alert
                type="error"
                message="Download failed"
                description={errorMsg}
                showIcon
                action={
                  <Button size="small" danger onClick={handleReset}>
                    Retry
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
