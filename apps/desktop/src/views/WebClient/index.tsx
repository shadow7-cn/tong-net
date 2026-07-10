import { ChangeEvent, UIEvent, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Alert, Button, Empty, Input, Progress, Spin, Tag, message as toast } from "antd";
import { ChevronDown, ChevronLeft, Paperclip, RotateCcw, Send, UserRoundPen, X } from "lucide-react";
import { getBootstrap } from "@/api/service";
import { listDevices, saveDeviceName, updateDeviceName } from "@/api/device";
import { listMessages } from "@/api/conversation";
import { sendTextMessage } from "@/api/message";
import { cancelTransfer, uploadConversationFile } from "@/api/file";
import DeviceAvatar from "@/components/DeviceAvatar";
import FileCard from "@/components/FileCard";
import { useDeviceIdentity } from "@/hooks/useDeviceIdentity";
import { useLanSocket } from "@/hooks/useLanSocket";
import { getAccessToken, setCurrentDeviceId } from "@/http";
import { useServiceStore } from "@/store";
import type { Device, Message } from "@/types/domain";
import { formatTime } from "@/utils/time";
import { createId } from "@/utils/id";
import { isNearScrollBottom } from "@/utils/scroll";
import { estimateRemainingSeconds, formatRemainingTime, formatTransferSpeed } from "@/utils/transfer";
import styles from "./index.module.less";

type UploadItem = {
  id: string;
  file: File;
  name: string;
  progress: number;
  speed: number;
  remaining: number;
  status: "running" | "failed" | "canceled";
};

function defaultNickname() {
  if (/iPhone/i.test(navigator.userAgent)) return "我的 iPhone 访问端";
  if (/iPad/i.test(navigator.userAgent)) return "我的 iPad 访问端";
  if (/Android/i.test(navigator.userAgent)) return "我的 Android 访问端";
  return "我的浏览器访问端";
}

type WebClientProps = { hostMode?: boolean };

export default function WebClient({ hostMode = false }: WebClientProps) {
  const [api, contextHolder] = toast.useMessage();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const shouldStickToBottomRef = useRef(true);
  const uploadControllersRef = useRef(new Map<string, AbortController>());
  const uploadSamplesRef = useRef(new Map<string, { time: number; bytes: number; speed: number }>());
  const messagesRef = useRef<Message[]>([]);
  const clientId = useDeviceIdentity();
  const serviceRunning = useServiceStore((state) => state.running);
  const [currentDevice, setCurrentDevice] = useState<Device>();
  const [devices, setDevices] = useState<Device[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [nickname, setNickname] = useState(() => localStorage.getItem("tong-net-device-name") ?? defaultNickname());
  const [selectedPeerId, setSelectedPeerId] = useState("host");
  const [mobileView, setMobileView] = useState<"list" | "chat">("list");
  const [loading, setLoading] = useState(true);
  const [fatalError, setFatalError] = useState("");
  const [uploads, setUploads] = useState<UploadItem[]>([]);
  const [newMessageCount, setNewMessageCount] = useState(0);

  const peers = devices.filter((device) => device.id !== currentDevice?.id);
  const selectedDevice = peers.find((device) => device.id === selectedPeerId) ?? peers[0];

  const refresh = useCallback(async () => {
    if (!currentDevice) return;
    const deviceResponse = await listDevices();
    setDevices(deviceResponse.data);
    const peerId = selectedDevice?.id ?? selectedPeerId;
    if (peerId) {
      const nextMessages = (await listMessages(peerId)).data;
      const knownIds = new Set(messagesRef.current.map((item) => item.id));
      const incomingCount = nextMessages.filter((item) => !knownIds.has(item.id) && item.fromDeviceId !== currentDevice.id).length;
      if (incomingCount > 0 && !shouldStickToBottomRef.current) {
        setNewMessageCount((count) => count + incomingCount);
      }
      messagesRef.current = nextMessages;
      setMessages(nextMessages);
    }
  }, [currentDevice, selectedDevice?.id, selectedPeerId]);

  useEffect(() => {
    if (hostMode) {
      if (!serviceRunning) {
        setFatalError("请先开启互通服务，再进入访问端会话。");
        setLoading(false);
        return;
      }
      setLoading(true);
      setFatalError("");
      listDevices().then(({ data }) => {
        const host = data.find((device) => device.id === "host");
        if (!host) throw new Error("未找到本机主机");
        setCurrentDevice(host);
        setCurrentDeviceId("host");
        setNickname(host.name);
        setDevices(data);
        const firstPeer = data.find((device) => device.id !== "host");
        if (firstPeer) setSelectedPeerId(firstPeer.id);
      }).catch((error) => setFatalError(String(error))).finally(() => setLoading(false));
      return;
    }
    saveDeviceName(nickname);
    if (!getAccessToken()) {
      setFatalError("访问地址缺少令牌，请重新扫描桌面端二维码或复制完整地址。");
      setLoading(false);
      return;
    }
    getBootstrap().then(({ data }) => {
      setCurrentDevice(data.currentDevice);
      setCurrentDeviceId(data.currentDevice.id);
      return listDevices();
    }).then(({ data }) => {
      setDevices(data);
      setLoading(false);
    }).catch((error) => {
      setFatalError(error.response?.data?.message ?? "无法连接同网互通主机");
      setLoading(false);
    });
  }, [clientId, hostMode, serviceRunning]);

  useEffect(() => {
    if (!currentDevice || !selectedDevice) return;
    shouldStickToBottomRef.current = true;
    setNewMessageCount(0);
    listMessages(selectedDevice.id).then(({ data }) => {
      messagesRef.current = data;
      setMessages(data);
    }).catch(() => undefined);
  }, [currentDevice, selectedDevice?.id]);

  useLayoutEffect(() => {
    const container = messageListRef.current;
    if (container && shouldStickToBottomRef.current) {
      container.scrollTop = container.scrollHeight;
    }
  }, [messages, mobileView, uploads]);

  const { connected } = useLanSocket(Boolean(currentDevice), currentDevice?.id ?? "", () => { void refresh(); });

  const deviceNameMap = useMemo(() => new Map(devices.map((device) => [device.id, device.name])), [devices]);

  const trackMessageScroll = (event: UIEvent<HTMLDivElement>) => {
    const atBottom = isNearScrollBottom(event.currentTarget);
    shouldStickToBottomRef.current = atBottom;
    if (atBottom) setNewMessageCount(0);
  };

  const scrollToLatest = () => {
    shouldStickToBottomRef.current = true;
    setNewMessageCount(0);
    const container = messageListRef.current;
    if (container) container.scrollTop = container.scrollHeight;
  };

  const followOwnMessage = () => {
    shouldStickToBottomRef.current = true;
    setNewMessageCount(0);
  };

  const sendMessage = async () => {
    const content = draft.trim();
    if (!content || !selectedDevice) return;
    followOwnMessage();
    setDraft("");
    try {
      const { data } = await sendTextMessage(selectedDevice.id, content);
      setMessages((items) => {
        const next = [...items, data];
        messagesRef.current = next;
        return next;
      });
    } catch (error: any) {
      setDraft(content);
      api.error(error.response?.data?.message ?? "消息发送失败");
    }
  };

  const changeNickname = async () => {
    const name = nickname.trim();
    if (!name) { api.warning("昵称不能为空"); return; }
    try {
      const { data } = await updateDeviceName(name);
      if (!hostMode) saveDeviceName(name);
      setCurrentDevice(data);
      setDevices((items) => items.map((item) => item.id === data.id ? data : item));
      api.success("昵称已更新");
    } catch (error: any) { api.error(error.response?.data?.message ?? "昵称更新失败"); }
  };

  const startUpload = (file: File) => {
    if (!selectedDevice) return;
    const peerId = selectedDevice.id;
    const id = createId();
    const controller = new AbortController();
    uploadControllersRef.current.set(id, controller);
    uploadSamplesRef.current.set(id, { time: performance.now(), bytes: 0, speed: 0 });
    setUploads((items) => [...items, { id, file, name: file.name, progress: 0, speed: 0, remaining: 0, status: "running" }]);
    const formData = new FormData();
    formData.append("file", file);
    void uploadConversationFile(peerId, formData, {
      transferId: id,
      fileName: file.name,
      fileSize: file.size,
      signal: controller.signal,
      onProgress: ({ loaded, total, progress }) => {
        const now = performance.now();
        const sample = uploadSamplesRef.current.get(id);
        let speed = sample?.speed ?? 0;
        if (sample && now - sample.time >= 200) {
          const instant = (loaded - sample.bytes) / ((now - sample.time) / 1000);
          speed = sample.speed ? sample.speed * 0.65 + instant * 0.35 : instant;
          uploadSamplesRef.current.set(id, { time: now, bytes: loaded, speed });
        }
        setUploads((items) => items.map((item) => item.id === id ? {
          ...item,
          progress,
          speed,
          remaining: estimateRemainingSeconds(total, loaded, speed),
        } : item));
      },
    }).then(({ data }) => {
      setMessages((items) => {
        const next = [...items, data];
        messagesRef.current = next;
        return next;
      });
      setUploads((items) => items.filter((item) => item.id !== id));
    }).catch((error: any) => {
      if (controller.signal.aborted) return;
      setUploads((items) => items.map((item) => item.id === id ? { ...item, status: "failed" } : item));
      api.error(`${file.name} 上传失败：${error.response?.data?.message ?? "连接中断"}`);
    }).finally(() => {
      uploadControllersRef.current.delete(id);
      uploadSamplesRef.current.delete(id);
    });
  };

  const cancelUpload = (id: string) => {
    setUploads((items) => items.map((item) => item.id === id ? { ...item, status: "canceled" } : item));
    const controller = uploadControllersRef.current.get(id);
    void cancelTransfer(id).finally(() => controller?.abort());
  };

  const retryUpload = (item: UploadItem) => {
    setUploads((items) => items.filter((upload) => upload.id !== item.id));
    startUpload(item.file);
  };

  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (!selectedDevice) return;
    followOwnMessage();
    files.forEach(startUpload);
  };

  if (loading) return <div className={`${styles.centerState} ${hostMode ? styles.desktopCenterState : ""}`}><Spin size="large" /></div>;
  if (fatalError) return <div className={`${styles.centerState} ${hostMode ? styles.desktopCenterState : ""}`}><Alert type="error" showIcon message="无法进入" description={fatalError} /></div>;

  return (
    <div className={`${styles.page} ${hostMode ? styles.desktopEmbedded : ""} ${mobileView === "chat" ? styles.mobileChat : styles.mobileList}`}>
      {contextHolder}
      <section className={styles.sidebar}>
        <div className={styles.mobileTitle}>
          <div className={styles.mobileBrand}>
            <img src="/brand/tong-net-logo.png" alt="同网互通 Logo" />
            <div><h1>同网互通</h1><p>选择一个访问端开始发送消息或文件。</p></div>
          </div>
          <Tag color={connected ? "green" : "orange"}>{connected ? "已连接" : "重连中"}</Tag>
        </div>
        {currentDevice && <div className={styles.profile}>
          <DeviceAvatar device={currentDevice} />
          <div className={styles.profileBody}><div className={styles.label}>当前访问端</div><Input size="small" value={nickname} maxLength={40} onChange={(event) => setNickname(event.target.value)} onPressEnter={changeNickname} suffix={<UserRoundPen size={14} />} /></div>
          <Button size="small" onClick={changeNickname}>保存</Button>
        </div>}
        <div className={styles.deviceList}>
          {peers.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无其他访问端" />}
          {peers.map((device) => <button type="button" key={device.id} className={`${styles.deviceButton} ${device.id === selectedDevice?.id ? styles.activeDevice : ""}`} onClick={() => { setSelectedPeerId(device.id); setMobileView("chat"); }}>
            <DeviceAvatar device={device} size="small" /><span><strong>{device.name}</strong><small>{device.kind === "host" ? "本机主机" : device.browserSource} · {device.status === "online" ? "在线" : "离线"}</small></span>
            <Tag color={device.status === "online" ? "green" : "default"}>{device.status === "online" ? "可连接" : "离线"}</Tag>
          </button>)}
        </div>
      </section>

      <section className={styles.chat}>
        {selectedDevice ? <>
          <header className={styles.chatHeader}>
            <Button className={styles.backButton} aria-label="返回访问端列表" type="text" icon={<ChevronLeft size={19} />} onClick={() => setMobileView("list")} />
            <div className={styles.chatPeer}><DeviceAvatar device={selectedDevice} /><div><h1>{selectedDevice.name}</h1><p>一对一会话，文件通过主机中转保存。</p></div></div>
            <Tag color={selectedDevice.status === "online" ? "green" : "default"}>{selectedDevice.status === "online" ? "在线" : "离线"}</Tag>
          </header>
          <div ref={messageListRef} data-testid="message-list" className={styles.messageList} onScroll={trackMessageScroll}>
            {messages.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="还没有消息" />}
            {messages.map((item) => { const mine = item.fromDeviceId === currentDevice?.id; return <div key={item.id} className={`${styles.messageRow} ${mine ? styles.mine : ""}`}><div className={styles.messageMeta}>{deviceNameMap.get(item.fromDeviceId) ?? "访问端"} · {formatTime(item.createdAt)}</div><div className={item.type === "system" ? styles.systemBubble : styles.bubble}>{item.file ? <FileCard file={item.file} hostMode={hostMode} /> : item.content}</div></div>; })}
            {uploads.map((item) => <div key={item.id} className={`${styles.messageRow} ${styles.mine}`}><div className={styles.messageMeta}>{item.name}</div><div className={styles.uploadBubble}>
              <Progress percent={item.progress} status={item.status === "failed" ? "exception" : item.status === "running" ? "active" : "normal"} size="small" />
              <span>{item.status === "running" ? `${formatTransferSpeed(item.speed)} ${formatRemainingTime(item.remaining)}` : item.status === "failed" ? "上传失败" : "已取消"}</span>
              {item.status === "running" ? <Button size="small" type="text" aria-label="取消传输" icon={<X size={15} />} onClick={() => cancelUpload(item.id)} /> : <Button size="small" type="link" icon={<RotateCcw size={14} />} onClick={() => retryUpload(item)}>重试</Button>}
              {item.status !== "running" && <Button size="small" type="link" onClick={() => setUploads((items) => items.filter((upload) => upload.id !== item.id))}>移除</Button>}
            </div></div>)}
          </div>
          {newMessageCount > 0 && (
            <Button className={styles.newMessageNotice} icon={<ChevronDown size={15} />} onClick={scrollToLatest}>
              有 {newMessageCount} 条新消息
            </Button>
          )}
          <footer className={styles.composer}>
            <input ref={fileInputRef} type="file" multiple className={styles.fileInput} onChange={handleFileChange} />
            <Button aria-label="选择文件" icon={<Paperclip size={16} />} onClick={() => fileInputRef.current?.click()} />
            <Input.TextArea value={draft} onChange={(event) => setDraft(event.target.value)} onPressEnter={(event) => { if (!event.shiftKey) { event.preventDefault(); void sendMessage(); } }} autoSize={{ minRows: 1, maxRows: 3 }} placeholder="输入消息，Enter 发送" />
            <Button type="primary" icon={<Send size={16} />} onClick={sendMessage}>发送</Button>
          </footer>
        </> : <Empty description="暂无可用访问端" />}
      </section>
    </div>
  );
}
