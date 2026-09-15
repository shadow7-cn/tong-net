import { useEffect, useRef, useState } from "react";
import { Alert, Button, QRCode, Space, Tabs, message } from "antd";
import { MessageCircle, FolderOpen, Play, QrCode, Square, Users } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import TransferProgress from "@/components/TransferProgress";
import { useDeviceStore, useEasyTierStore, useServiceStore, useTransferStore } from "@/store";
import { buildAccessUrl } from "@/utils/accessUrl";
import { openSaveDirectory } from "@/api/service";
import { cancelTransfer } from "@/api/file";
import styles from "./index.module.less";

export default function DesktopHome() {
  const [api, contextHolder] = message.useMessage();
  const navigate = useNavigate();
  const { running, loading, tokenRequired, startedAt, lanUrl, port, token, startService, stopService } = useServiceStore();
  const { connected, virtualIp, refresh: refreshEasyTier } = useEasyTierStore();
  const [accessMode, setAccessMode] = useState("lan");
  const virtualAvailable = connected && Boolean(virtualIp);
  const selectedMode = running && virtualAvailable ? accessMode : "lan";
  const joinUrl = running ? selectedMode === "virtual" ? buildAccessUrl(virtualIp, port, token, tokenRequired) : lanUrl : "";
  const devices = useDeviceStore((state) => state.devices);
  const loadDevices = useDeviceStore((state) => state.loadDevices);
  const transfers = useTransferStore((state) => state.transfers);
  const loadTransfers = useTransferStore((state) => state.loadTransfers);
  const samples = useRef(new Map<string, { bytes: number; time: number }>());
  const [speeds, setSpeeds] = useState<Record<string, number>>({});
  const [now, setNow] = useState(Date.now());
  const [canceling, setCanceling] = useState<string[]>([]);
  const active = running ? transfers.filter((task) => task.status === "running") : [];
  const onlineCount = running ? devices.filter((device) => device.status === "online" && device.kind !== "host").length : 0;
  const elapsed = running && startedAt ? Math.max(0, Math.floor((now - new Date(startedAt).getTime()) / 60000)) : 0;
  const duration = elapsed >= 60 ? `${Math.floor(elapsed / 60)} 小时 ${elapsed % 60} 分钟` : `${elapsed} 分钟`;

  useEffect(() => {
    if (!virtualAvailable) setAccessMode("lan");
  }, [virtualAvailable]);

  useEffect(() => {
    if (!running) return;
    let pending = false;
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try { await refreshEasyTier(); }
      catch { /* Retry after the next status refresh. */ }
      finally { pending = false; }
    };
    void refresh();
    const timer = window.setInterval(refresh, 1500);
    return () => window.clearInterval(timer);
  }, [running, refreshEasyTier]);

  useEffect(() => {
    if (!running) return;
    let pending = false;
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try { await Promise.all([loadDevices(), loadTransfers()]); setNow(Date.now()); }
      catch { /* The next refresh retries after reconnecting. */ }
      finally { pending = false; }
    };
    void refresh();
    const timer = window.setInterval(refresh, 1000);
    return () => window.clearInterval(timer);
  }, [loadDevices, loadTransfers, running]);

  useEffect(() => {
    const time = performance.now();
    const next: Record<string, number> = {};
    const nextSamples = new Map<string, { bytes: number; time: number }>();
    for (const task of transfers.filter((item) => item.status === "running")) {
      const bytes = task.transferredBytes ?? 0;
      const sample = samples.current.get(task.id);
      next[task.id] = sample && time > sample.time ? Math.max(0, (bytes - sample.bytes) * 1000 / (time - sample.time)) : 0;
      nextSamples.set(task.id, { bytes, time });
    }
    samples.current = nextSamples;
    setSpeeds(next);
  }, [transfers]);

  const toggleService = async () => {
    try { if (running) await stopService(); else await startService(); }
    catch (error) { api.error(String(error)); }
  };

  const cancel = async (id: string, kind: string) => {
    setCanceling((items) => [...items, id]);
    try {
      if (kind === "download") await invoke("cancel_native_transfer", { transferId: id });
      else await cancelTransfer(id);
      await loadTransfers();
    } catch { api.error("取消失败，请重试"); }
    finally { setCanceling((items) => items.filter((item) => item !== id)); }
  };

  return <div className={styles.page}>
    {contextHolder}
    <div className={styles.content}>
    <header className={styles.header}>
      <h1>互通服务</h1>
      <Button type="text" icon={<FolderOpen size={16} />} onClick={() => openSaveDirectory().catch((error) => api.error(String(error)))}>打开保存目录</Button>
    </header>
    <section className={styles.serviceSection} aria-label="互通服务控制">
      <div className={styles.controls}>
        <h2 className={styles.status}><span className={running ? styles.onlineDot : styles.offlineDot} />{running ? "互通运行中" : "互通未开启"}</h2>
        <p className={styles.duration}>{running ? `已运行 ${duration}` : "等待开启"}</p>
        <div className={styles.onlineCount}><Users size={18} /><strong>{onlineCount}</strong><span>个在线访问端</span></div>
        <Space wrap size={12}>
          <Button type={running ? "default" : "primary"} icon={running ? <Square size={15} /> : <Play size={15} />} loading={loading} onClick={toggleService}>{running ? "停止互通" : "开启互通"}</Button>
          <Button type={running ? "primary" : "default"} disabled={!running} icon={<MessageCircle size={16} />} onClick={() => navigate("/chat")}>进入群聊</Button>
        </Space>
        {running && !tokenRequired && <Alert className={styles.warning} type="warning" showIcon title="当前允许无令牌访问，同一局域网内的任何访问端都能连接、聊天和传输文件。" />}
      </div>
      <div className={styles.qrArea}>
        <Tabs size="small" activeKey={selectedMode} onChange={setAccessMode} items={[
          { key: "lan", label: "局域网" },
          { key: "virtual", label: "虚拟局域网", disabled: !running || !virtualAvailable },
        ]} />
        <div className={styles.qrFrame}>
          {joinUrl ? <QRCode value={joinUrl} size={180} errorLevel="M" bordered={false} /> : <QrCode size={56} aria-hidden="true" />}
        </div>
        <p className={styles.qrCaption}>{running ? joinUrl ? "扫码加入群聊" : "正在获取接入二维码" : "开启互通后可扫码加入"}</p>
      </div>
    </section>
    {active.length > 0 && <section className={styles.section}>
      <h2>正在传输 <span>{active.length}</span></h2>
      <div className={styles.activeList}>{active.map((task) => <TransferProgress key={task.id} task={task} speed={speeds[task.id] ?? 0} canceling={canceling.includes(task.id)} onCancel={task.kind === "upload" || task.peerName === "本机另存" ? () => cancel(task.id, task.kind) : undefined} />)}</div>
    </section>}
    </div>
  </div>;
}
