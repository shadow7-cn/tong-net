import { useEffect, useState } from "react";
import { Alert, Badge, Button, Empty, Input, Popconfirm, Space, Tabs, Tag, Tooltip, message } from "antd";
import QRCode from "qrcode";
import { CheckCircle2, Copy, FolderOpen, Network, Play, QrCode, Square, Trash2, Wifi } from "lucide-react";
import DeviceAvatar from "@/components/DeviceAvatar";
import TransferProgress from "@/components/TransferProgress";
import { useDeviceStore, useEasyTierStore, useServiceStore, useTransferStore, useUnreadStore } from "@/store";
import { openSaveDirectory } from "@/api/service";
import { removeDevice } from "@/api/device";
import { formatDateTime } from "@/utils/time";
import { buildAccessUrl } from "@/utils/accessUrl";
import styles from "./index.module.less";

export default function DesktopHome() {
  const [api, contextHolder] = message.useMessage();
  const [qrUrl, setQrUrl] = useState("");
  const [qrMode, setQrMode] = useState<"lan" | "virtual">("lan");
  const { running, loading, lanUrl, port, token, tokenRequired, startedAt, startService, stopService } = useServiceStore();
  const easyTierConnected = useEasyTierStore((state) => state.connected);
  const easyTierVirtualIp = useEasyTierStore((state) => state.virtualIp);
  const refreshEasyTier = useEasyTierStore((state) => state.refresh);
  const devices = useDeviceStore((state) => state.devices);
  const loadDevices = useDeviceStore((state) => state.loadDevices);
  const transfers = useTransferStore((state) => state.transfers);
  const loadTransfers = useTransferStore((state) => state.loadTransfers);
  const unreadByPeer = useUnreadStore((state) => state.unreadByPeer);
  const onlineCount = devices.filter((device) => device.status === "online").length;
  const virtualLanUrl = running && easyTierConnected
    ? buildAccessUrl(easyTierVirtualIp, port, token, tokenRequired)
    : "";
  const qrTarget = qrMode === "virtual" ? virtualLanUrl : lanUrl;

  const copyUrl = async (url: string, label: string) => {
    if (!url) return;
    await navigator.clipboard.writeText(url);
    api.success(`${label}已复制`);
  };

  useEffect(() => {
    if (!running || !qrTarget) { setQrUrl(""); return; }
    QRCode.toDataURL(qrTarget, { width: 196, margin: 1, errorCorrectionLevel: "M" }).then(setQrUrl).catch(() => setQrUrl(""));
    const refresh = () => Promise.all([loadDevices(), loadTransfers()]).catch(() => undefined);
    void refresh();
    const timer = window.setInterval(refresh, 3000);
    return () => window.clearInterval(timer);
  }, [loadDevices, loadTransfers, qrTarget, running]);

  useEffect(() => {
    void refreshEasyTier().catch(() => undefined);
    const timer = window.setInterval(() => void refreshEasyTier().catch(() => undefined), 1500);
    return () => window.clearInterval(timer);
  }, [refreshEasyTier]);

  useEffect(() => {
    if (!virtualLanUrl && qrMode === "virtual") setQrMode("lan");
  }, [qrMode, virtualLanUrl]);

  const toggleService = async () => {
    try {
      if (running) await stopService(); else await startService();
      api.success(running ? "互通服务已停止" : "互通服务已开启");
    } catch (error) { api.error(String(error)); }
  };

  const removeAccessClient = async (deviceId: string) => {
    try {
      await removeDevice(deviceId);
      await loadDevices();
      api.success("访问端已移除，历史记录仍会保留");
    } catch (error: any) {
      api.error(error.response?.data?.message ?? "访问端移除失败");
    }
  };

  return (
    <div className={styles.page}>
      {contextHolder}
      <header className={styles.header}>
        <div>
          <h1>App 端控制台</h1>
          <p>开启服务后，同一局域网内的访问端可以扫码或复制地址进入 Web 端。</p>
        </div>
        <Space>
          <Button icon={<FolderOpen size={16} />} onClick={() => openSaveDirectory().catch((error) => api.error(String(error)))}>打开保存目录</Button>
          <Button
            type={running ? "default" : "primary"}
            icon={running ? <Square size={15} /> : <Play size={15} />}
            loading={loading}
            onClick={toggleService}
          >
            {running ? "停止互通" : "开启互通"}
          </Button>
        </Space>
      </header>

      <Alert
        className={styles.securityNotice}
        type={tokenRequired ? "warning" : "error"}
        showIcon
        message={tokenRequired
          ? "仅在可信局域网中开启。访问地址包含临时令牌，请不要转发给不信任的人。"
          : "当前允许无令牌访问，同一局域网内的任何访问端都能连接、聊天和传输文件。"}
      />

      <section className={styles.hero}>
        <div className={styles.statusPanel}>
          <div className={styles.statusTop}>
            <span className={running ? styles.pulseOn : styles.pulseOff} />
            <span>{running ? "局域网服务运行中" : "局域网服务未开启"}</span>
            <Tag color={running ? "green" : "default"}>端口 {port}</Tag>
          </div>
          <div className={styles.addressList}>
            <div className={styles.urlRow}>
              <span>局域网地址</span>
              <Input value={lanUrl} readOnly disabled={!running} placeholder="开启服务后生成局域网地址" />
              <Tooltip title="复制局域网地址">
                <Button icon={<Copy size={16} />} disabled={!lanUrl} onClick={() => void copyUrl(lanUrl, "局域网地址")} />
              </Tooltip>
            </div>
            <div className={styles.urlRow}>
              <span>虚拟局域网地址</span>
              <Input
                value={virtualLanUrl}
                readOnly
                disabled={!virtualLanUrl}
                placeholder={running ? "连接虚拟局域网并获取 IP 后可用" : "请先开启互通服务"}
              />
              <Tooltip title="复制虚拟局域网地址">
                <Button
                  icon={<Copy size={16} />}
                  disabled={!virtualLanUrl}
                  onClick={() => void copyUrl(virtualLanUrl, "虚拟局域网地址")}
                />
              </Tooltip>
            </div>
          </div>
          <div className={styles.metrics}>
            <div>
              <strong>{onlineCount}</strong>
              <span>在线访问端</span>
            </div>
            <div>
              <strong>{transfers.length}</strong>
              <span>传输记录</span>
            </div>
            <div>
              <strong>{startedAt ? formatDateTime(startedAt) : "--"}</strong>
              <span>开启时间</span>
            </div>
          </div>
        </div>
        <div className={styles.qrPanel}>
          <div className={styles.qrTitle}>
            <QrCode size={18} />
            手机扫码进入
          </div>
          <Tabs
            size="small"
            activeKey={qrMode}
            onChange={(key) => setQrMode(key as "lan" | "virtual")}
            centered
            items={[
              { key: "lan", label: "局域网" },
              {
                key: "virtual",
                label: <Space size={4}><Network size={13} />虚拟局域网</Space>,
                disabled: !virtualLanUrl,
              },
            ]}
          />
          {qrUrl ? <img className={styles.qrImage} src={qrUrl} alt={`${qrMode === "virtual" ? "虚拟局域网" : "局域网"}访问二维码`} /> : <div className={styles.qrPlaceholder}><QrCode size={44} /></div>}
          <p>二维码会携带本次会话访问令牌。</p>
        </div>
      </section>

      <section className={styles.grid}>
        <div className={styles.panel}>
          <div className={styles.panelTitle}>
            <Wifi size={17} />
            访问端列表
          </div>
          <div className={styles.deviceList}>
            {devices.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={running ? "暂无访问端" : "服务未开启"} />}
            {devices.map((device) => (
              <div key={device.id} className={styles.deviceItem}>
                <Badge count={unreadByPeer[device.id] ?? 0} overflowCount={99} size="small">
                  <DeviceAvatar device={device} />
                </Badge>
                <div>
                  <div className={styles.deviceName}>{device.name}</div>
                  <div className={styles.deviceMeta}>{device.kind === "host" ? "本机主机" : device.browserSource}</div>
                </div>
                <div className={styles.deviceActions}>
                  <Tag color={device.status === "online" ? "green" : "default"}>
                    {device.status === "online" ? "在线" : "离线"}
                  </Tag>
                  {device.kind === "browser" && device.status === "offline" && (
                    <Popconfirm
                      title="移除这个访问端？"
                      description="将从访问端和会话列表隐藏，聊天与文件记录仍会保留。"
                      okText="移除"
                      cancelText="取消"
                      okButtonProps={{ danger: true }}
                      onConfirm={() => removeAccessClient(device.id)}
                    >
                      <Tooltip title="移除访问端">
                        <Button danger type="text" aria-label={`移除访问端 ${device.name}`} icon={<Trash2 size={16} />} />
                      </Tooltip>
                    </Popconfirm>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className={styles.panel}>
          <div className={styles.panelTitle}>
            <CheckCircle2 size={17} />
            最近传输
          </div>
          <div className={styles.transferList}>
            {transfers.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无传输" />}
            {transfers.map((task) => (
              <TransferProgress key={task.id} task={task} />
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
