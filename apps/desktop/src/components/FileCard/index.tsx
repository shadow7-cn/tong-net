import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { Button, Image, Popconfirm, Progress, Tag, Tooltip, message } from "antd";
import { Download, FileArchive, FolderOpen, Link, X } from "lucide-react";
import { getDownloadUrl, getFileAvailability } from "@/api/file";
import type { FileRecord } from "@/types/domain";
import { formatFileSize } from "@/utils/fileSize";
import { copyText } from "@/utils/clipboard";
import { createId } from "@/utils/id";
import { isPreviewImage } from "@/utils/filePreview";
import { formatRemainingTime, formatTransferSpeed, estimateRemainingSeconds } from "@/utils/transfer";
import styles from "./index.module.less";

type FileCardProps = {
  file: FileRecord;
  hostMode?: boolean;
};

type NativeProgress = { transferId: string; transferredBytes: number; totalBytes: number };

export default function FileCard({ file, hostMode = false }: FileCardProps) {
  const [missing, setMissing] = useState(file.status === "missing");
  const downloadUrl = file.status !== "failed" && !missing ? getDownloadUrl(file.id) : "";
  const imageUrl = downloadUrl && isPreviewImage(file.name) ? getDownloadUrl(file.id, true) : "";
  const [previewFailed, setPreviewFailed] = useState(false);
  const [task, setTask] = useState<{ id: string; progress: number; speed: number; remaining: number; status: "running" | "failed" | "canceled" }>();
  const sampleRef = useRef({ time: 0, bytes: 0, speed: 0 });
  const [opening, setOpening] = useState(false);
  const checkFile = useCallback(async () => {
    const { data } = await getFileAvailability(file.id);
    setMissing(!data.exists);
    return data.exists;
  }, [file.id]);
  useEffect(() => { setPreviewFailed(false); }, [missing, file.id]);
  useEffect(() => {
    setMissing(file.status === "missing");
    const refresh = () => { void checkFile().catch(() => undefined); };
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [checkFile, file.status]);
  const openFile = async () => {
    setOpening(true);
    try { if (await checkFile()) await invoke("open_shared_file", { fileId: file.id }); }
    catch (error) { message.error(String(error)); }
    finally { setOpening(false); }
  };

  const copyDownloadUrl = async () => {
    try {
      if (!await checkFile()) return;
      await copyText(downloadUrl);
      message.success("下载链接已复制，请仅发送到可信访问端");
    } catch {
      message.error("复制失败，请长按另存为按钮复制链接");
    }
  };

  const saveAs = async () => {
    let destination: string | null;
    try { if (!await checkFile()) return; destination = await save({ title: "选择文件保存位置", defaultPath: file.name }); }
    catch (error) { message.error(`无法选择保存位置：${String(error)}`); return; }
    if (!destination) return;
    const id = createId();
    sampleRef.current = { time: performance.now(), bytes: 0, speed: 0 };
    setTask({ id, progress: 0, speed: 0, remaining: 0, status: "running" });
    let unlisten: (() => void) | undefined;
    try {
    unlisten = await listen<NativeProgress>("native-transfer-progress", ({ payload }) => {
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
      await invoke("save_file_as", { fileId: file.id, destination, transferId: id });
      setTask(undefined);
      message.success(`${file.name} 已保存`);
    } catch (error) {
      const canceled = String(error).includes("取消");
      setTask((current) => current ? { ...current, status: canceled ? "canceled" : "failed" } : current);
      if (!canceled) message.error(`${file.name} 保存失败：${String(error)}`);
    } finally {
      unlisten?.();
    }
  };

  const cancelNativeSave = async () => {
    if (!task) return;
    try { await invoke("cancel_native_transfer", { transferId: task.id }); }
    catch (error) { message.error(String(error)); }
  };

  return (
    <div className={styles.card}>
      {imageUrl && <div className={styles.imagePreview}>
        {previewFailed ? <span>图片预览失败，可另存为后查看</span> : <Image src={imageUrl} alt={file.name} width="100%" height={220} loading="lazy" preview={{ motionName: "" }} referrerPolicy="no-referrer" onError={() => { setPreviewFailed(true); void checkFile().catch(() => undefined); }} />}
      </div>}
      {missing && isPreviewImage(file.name) && <div className={styles.imagePreview}>文件不存在</div>}
      <div className={styles.icon}>
        <FileArchive size={20} />
      </div>
      <div className={styles.body}>
        <div className={styles.name}>{file.name}</div>
        <div className={styles.meta}>
          {formatFileSize(file.size)}
          <Tag color={downloadUrl ? "green" : "red"}>
            {missing ? "文件不存在" : downloadUrl ? (hostMode ? "已保存" : "可保存") : "失败"}
          </Tag>
        </div>
      </div>
      <div className={styles.actions}>
        {hostMode ? (
          <>
          <Popconfirm title="使用系统默认应用打开？" description="请仅打开来自可信发送者的文件。" onConfirm={openFile} okText="打开" cancelText="取消" disabled={!downloadUrl}>
            <Button icon={<FolderOpen size={16} />} disabled={!downloadUrl} loading={opening}>打开</Button>
          </Popconfirm>
          <Button icon={<Download size={16} />} disabled={!downloadUrl || task?.status === "running"} onClick={saveAs}>另存为</Button>
          </>
        ) : (
          <>
            <Button icon={<Download size={16} />} href={downloadUrl || undefined} target="_blank" rel="noopener noreferrer" disabled={!downloadUrl} onClick={async (event) => {
              event.preventDefault();
              const target = window.open("about:blank", "_blank");
              if (target) target.opener = null;
              try {
                if (await checkFile()) {
                  if (target) target.location.href = downloadUrl;
                  else message.warning("请允许浏览器弹出下载窗口后重试");
                } else target?.close();
              } catch { target?.close(); message.error("无法检查文件，请稍后重试"); }
            }}>另存为</Button>
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
