import { useEffect } from "react";
import { Modal, message } from "antd";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useUpdateStore } from "@/store/update";
import styles from "./index.module.less";

export default function UpdateNotice() {
  const { start, availableUpdate, dismiss } = useUpdateStore();
  const [api, contextHolder] = message.useMessage();
  useEffect(() => { start(); }, [start]);

  const openRelease = async () => {
    if (!availableUpdate) return;
    try {
      await openUrl(availableUpdate.releaseUrl);
      dismiss();
    } catch {
      api.error("无法打开浏览器，请稍后重试");
    }
  };

  return <>
    {contextHolder}
    <Modal title={`发现新版本 v${availableUpdate?.latestVersion ?? ""}`}
      open={Boolean(availableUpdate)} onCancel={dismiss} onOk={openRelease}
      okText="前往下载" cancelText="稍后再说">
      <p>当前版本：v{availableUpdate?.currentVersion}</p>
      <div className={styles.notes}>{availableUpdate?.notes.trim() || "此版本暂无更新说明。"}</div>
    </Modal>
  </>;
}
