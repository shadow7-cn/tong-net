import { Button, Tag, Tooltip, message } from "antd";
import { Download, FileArchive, Link } from "lucide-react";
import { getDownloadUrl } from "@/api/file";
import type { FileRecord } from "@/types/domain";
import { formatFileSize } from "@/utils/fileSize";
import { copyText } from "@/utils/clipboard";
import styles from "./index.module.less";

type FileCardProps = {
  file: FileRecord;
};

export default function FileCard({ file }: FileCardProps) {
  const downloadUrl = file.status === "available" ? getDownloadUrl(file.id) : "";

  const copyDownloadUrl = async () => {
    try {
      await copyText(downloadUrl);
      message.success("下载链接已复制，请仅发送到可信访问端");
    } catch {
      message.error("复制失败，请长按下载按钮复制链接");
    }
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
        <Button icon={<Download size={16} />} href={downloadUrl || undefined} disabled={!downloadUrl}>下载</Button>
        <Tooltip title="复制下载链接">
          <Button aria-label="复制下载链接" icon={<Link size={16} />} disabled={!downloadUrl} onClick={copyDownloadUrl} />
        </Tooltip>
      </div>
    </div>
  );
}
