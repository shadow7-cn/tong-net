import { Progress, Tag } from "antd";
import { ArrowDownToLine, ArrowUpFromLine } from "lucide-react";
import type { TransferTask } from "@/types/domain";
import styles from "./index.module.less";

type TransferProgressProps = {
  task: TransferTask;
};

export default function TransferProgress({ task }: TransferProgressProps) {
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
        <div className={styles.peer}>{task.kind === "upload" ? "发给" : "来自"} {task.peerName}</div>
        <Progress percent={task.progress} strokeColor={color} size="small" />
      </div>
    </div>
  );
}
