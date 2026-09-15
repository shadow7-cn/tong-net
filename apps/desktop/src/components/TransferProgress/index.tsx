import { Button, Progress, Tag, Tooltip } from "antd";
import { ArrowDownToLine, ArrowUpFromLine, X } from "lucide-react";
import { formatTransferSpeed } from "@/utils/transfer";
import { formatDateTime } from "@/utils/time";
import type { TransferTask } from "@/types/domain";
import styles from "./index.module.less";

type TransferProgressProps = {
  task: TransferTask;
  speed?: number;
  canceling?: boolean;
  onCancel?: () => void;
};

export default function TransferProgress({ task, speed = 0, canceling, onCancel }: TransferProgressProps) {
  const Icon = task.kind === "upload" ? ArrowUpFromLine : ArrowDownToLine;
  const color = task.status === "failed" ? "#dc2626" : task.status === "canceled" ? "#64748b" : task.status === "success" ? "#16a34a" : "#2563eb";
  const statusText = task.status === "failed" ? "失败" : task.status === "canceled" ? "已取消" : task.status === "success" ? "完成" : "传输中";

  return (
    <div className={styles.task}>
      <div className={styles.icon}>
        <Icon size={16} />
      </div>
      <div className={styles.body}>
        <div className={styles.topline}>
          <span className={styles.name}>{task.fileName}</span>
          <Tag color={task.status === "failed" ? "red" : task.status === "canceled" ? "default" : task.status === "success" ? "green" : "blue"}>
            {statusText}
          </Tag>
        </div>
        <div className={styles.peer}>{task.kind === "upload" ? "上传" : "下载"} · {task.peerName}{task.createdAt ? ` · ${formatDateTime(task.createdAt)}` : ""}</div>
        {task.status === "running" && <div className={styles.progressRow}>
          <Progress percent={task.progress} strokeColor={color} size="small" />
          <span className={styles.speed}>{formatTransferSpeed(speed)}</span>
          {onCancel && <Tooltip title="取消传输"><Button aria-label={`取消传输 ${task.fileName}`} type="text" loading={canceling} icon={<X size={16} />} onClick={onCancel} /></Tooltip>}
        </div>}
      </div>
    </div>
  );
}
