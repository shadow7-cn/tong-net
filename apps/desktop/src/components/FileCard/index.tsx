import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { Button, Progress, Tag, Tooltip, message } from "antd";
import { Download, FileArchive, Link, X } from "lucide-react";
import { getDownloadUrl } from "@/api/file";
import type { FileRecord } from "@/types/domain";
import { formatFileSize } from "@/utils/fileSize";
import { copyText } from "@/utils/clipboard";
import { createId } from "@/utils/id";
import { formatRemainingTime, formatTransferSpeed, estimateRemainingSeconds } from "@/utils/transfer";
import styles from "./index.module.less";

type FileCardProps = {
  file: FileRecord;
  hostMode?: boolean;
};

type NativeProgress = { transferId: string; transferredBytes: number; totalBytes: number };

export default function FileCard({ file, hostMode = false }: FileCardProps) {
  const downloadUrl = file.status === "available" ? getDownloadUrl(file.id) : "";
  const [task, setTask] = useState<{ id: string; progress: number; speed: number; remaining: number; status: "running" | "failed" | "canceled" }>();
  const sampleRef = useRef({ time: 0, bytes: 0, speed: 0 });

  const copyDownloadUrl = async () => {
    try {
      await copyText(downloadUrl);
      message.success("下载链接已复制，请仅发送到可信访问端");
    } catch {
      message.error("复制失败，请长按下载按钮复制链接");
    }
  };

  const saveAs = async () => {
    const destination = await save({ title: "选择文件保存位置", defaultPath: file.name });
    if (!destination) return;
    const id = createId();
    sampleRef.current = { time: performance.now(), bytes: 0, speed: 0 };
    setTask({ id, progress: 0, speed: 0, remaining: 0, status: "running" });
    const unlisten = await listen<NativeProgress>("native-transfer-progress", ({ payload }) => {
      if (payload.transferId !== id) return;
      const now = performance.now();
      const elapsed = (now - sampleRef.current.time) / 1000;
      if (elapsed >= 0.2) {
        const instant = (payload.transferredBytes - sampleRef.current.bytes) / elapsed;
        const speed = sampleRef.current.speed ? sampleRef.current.speed * 0.65 + instant * 0.35 : instant;
        sampleRef.current = { time: now, bytes: payload.transferredBytes, speed };
      }
      const speed = sampleRef.current.speed;
      setTask((current) => current?.id === id ? {
        ...current,
        progress: payload.totalBytes ? Math.round(payload.transferredBytes / payload.totalBytes * 100) : 0,
        speed,
        remaining: estimateRemainingSeconds(payload.totalBytes, payload.transferredBytes, speed),
      } : current);
    });
    try {
      await invoke("save_file_as", { fileId: file.id, destination, transferId: id });
      setTask(undefined);
      message.success(`${file.name} 已保存`);
    } catch (error) {
      const canceled = String(error).includes("取消");
      setTask((current) => current ? { ...current, status: canceled ? "canceled" : "failed" } : current);
      if (!canceled) message.error(`${file.name} 保存失败：${String(error)}`);
    } finally {
      unlisten();
    }
  };

  const cancelNativeSave = async () => {
    if (!task) return;
    await invoke("cancel_native_transfer", { transferId: task.id });
  };

  return (
    <div className={styles.card}>
      <div className={styles.icon}>
        <FileArchive size={20} />
      </div>
      <div className={styles.body}>
        <div className={styles.name}>{file.name}</div>
        <div className={styles.meta}>
          {formatFileSize(file.size)}
          <Tag color={file.status === "available" ? "green" : "red"}>
            {file.status === "available" ? "可下载" : "失败"}
          </Tag>
        </div>
      </div>
      <div className={styles.actions}>
        {hostMode ? (
          <Button icon={<Download size={16} />} disabled={!downloadUrl || task?.status === "running"} onClick={saveAs}>另存为</Button>
        ) : (
          <>
            <Button icon={<Download size={16} />} href={downloadUrl || undefined} disabled={!downloadUrl}>下载</Button>
            <Tooltip title="复制下载链接">
              <Button aria-label="复制下载链接" icon={<Link size={16} />} disabled={!downloadUrl} onClick={copyDownloadUrl} />
            </Tooltip>
          </>
        )}
      </div>
      {task && <div className={styles.transfer}>
        <Progress percent={task.progress} status={task.status === "failed" ? "exception" : task.status === "canceled" ? "normal" : "active"} size="small" />
        <span>{task.status === "running" ? `${formatTransferSpeed(task.speed)} ${formatRemainingTime(task.remaining)}` : task.status === "canceled" ? "已取消" : "保存失败"}</span>
        {task.status === "running" && <Tooltip title="取消传输"><Button aria-label="取消传输" size="small" type="text" icon={<X size={15} />} onClick={cancelNativeSave} /></Tooltip>}
        {task.status !== "running" && <Button size="small" type="link" onClick={() => setTask(undefined)}>关闭</Button>}
      </div>}
    </div>
  );
}
