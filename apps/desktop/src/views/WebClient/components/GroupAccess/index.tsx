import { useEffect } from "react";
import { Button, Tooltip, message } from "antd";
import { Copy } from "lucide-react";
import { useEasyTierStore, useServiceStore } from "@/store";
import { buildAccessUrl } from "@/utils/accessUrl";
import { copyText } from "@/utils/clipboard";
import JoinGroupPopover from "../JoinGroupPopover";
import styles from "./index.module.less";

export default function GroupAccess() {
  const { lanUrl, port, token, tokenRequired } = useServiceStore();
  const { connected, virtualIp, refresh } = useEasyTierStore();
  const [api, contextHolder] = message.useMessage();
  useEffect(() => {
    let pending = false;
    const update = async () => {
      if (pending) return;
      pending = true;
      try { await refresh(); } catch { /* Retry on the next status refresh. */ }
      finally { pending = false; }
    };
    void update();
    const timer = window.setInterval(update, 1500);
    return () => window.clearInterval(timer);
  }, [refresh]);
  const virtualUrl = connected ? buildAccessUrl(virtualIp, port, token, tokenRequired) : "";
  const addresses = [
    { label: "局域网", url: lanUrl },
    ...(virtualUrl ? [{ label: "虚拟局域网", url: virtualUrl }] : []),
  ].filter((item) => item.url);
  const copy = async (url: string) => {
    try { await copyText(url); api.success("入群地址已复制"); }
    catch { api.error("复制失败，请从二维码弹窗中手动复制"); }
  };
  return <div className={styles.addresses}>
    {contextHolder}
    {addresses.map(({ label, url }) => <div key={label} className={styles.row}>
      <span className={styles.label}>{label}</span>
      <span className={styles.host}>{new URL(url).host}</span>
      <Tooltip title={`复制${label}入群地址`}><Button size="small" type="text" aria-label={`复制${label}入群地址`} icon={<Copy size={14} />} onClick={() => copy(url)} /></Tooltip>
      <JoinGroupPopover url={url} label={`${label}地址`} />
    </div>)}
  </div>;
}
