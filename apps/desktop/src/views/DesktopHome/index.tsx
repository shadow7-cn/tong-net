import { useEffect, useState } from "react";
import { Alert, Button, Empty, Input, Popconfirm, Space, Tag, Tooltip, message } from "antd";
import QRCode from "qrcode";
import { CheckCircle2, Copy, FolderOpen, Play, QrCode, Square, Trash2, Wifi } from "lucide-react";
import DeviceAvatar from "@/components/DeviceAvatar";
import TransferProgress from "@/components/TransferProgress";
import { useDeviceStore, useServiceStore, useTransferStore } from "@/store";
import { openSaveDirectory } from "@/api/service";
import { removeDevice } from "@/api/device";
import { formatDateTime } from "@/utils/time";
import styles from "./index.module.less";

export default function DesktopHome() {
  const [api, contextHolder] = message.useMessage();
  const [qrUrl, setQrUrl] = useState("");
  const { running, loading, lanUrl, port, startedAt, startService, stopService } = useServiceStore();
  const devices = useDeviceStore((state) => state.devices);
  const loadDevices = useDeviceStore((state) => state.loadDevices);
  const transfers = useTransferStore((state) => state.transfers);
  const loadTransfers = useTransferStore((state) => state.loadTransfers);
  const onlineCount = devices.filter((device) => device.status === "online").length;

  const copyUrl = async () => {
    if (!lanUrl) return;
    await navigator.clipboard.writeText(lanUrl);
    api.success("局域网地址已复制");
  };

  useEffect(() => {
    if (!running) { setQrUrl(""); return; }
    QRCode.toDataURL(lanUrl, { width: 196, margin: 1, errorCorrectionLevel: "M" }).then(setQrUrl).catch(() => setQrUrl(""));
    const refresh = () => Promise.all([loadDevices(), loadTransfers()]).catch(() => undefined);
    void refresh();
    const timer = window.setInterval(refresh, 3000);
    return () => window.clearInterval(timer);
  }, [lanUrl, loadDevices, loadTransfers, running]);

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

      <Alert className={styles.securityNotice} type="warning" showIcon message="仅在可信局域网中开启。访问地址包含临时令牌，请不要转发给不信任的人。" />

      <section className={styles.hero}>
        <div className={styles.statusPanel}>
          <div className={styles.statusTop}>
            <span className={running ? styles.pulseOn : styles.pulseOff} />
            <span>{running ? "局域网服务运行中" : "局域网服务未开启"}</span>
            <Tag color={running ? "green" : "default"}>端口 {port}</Tag>
          </div>
          <div className={styles.urlRow}>
            <Input value={lanUrl} readOnly placeholder="开启服务后生成局域网地址" />
            <Tooltip title="复制地址">
              <Button icon={<Copy size={16} />} onClick={copyUrl} />
            </Tooltip>
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
          {qrUrl ? <img className={styles.qrImage} src={qrUrl} alt="局域网访问二维码" /> : <div className={styles.qrPlaceholder}><QrCode size={44} /></div>}
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
                <DeviceAvatar device={device} />
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
          {transfers.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无传输" />}
          {transfers.map((task) => (
            <TransferProgress key={task.id} task={task} />
          ))}
        </div>
      </section>
    </div>
  );
}
