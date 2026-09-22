import { ChangeEvent, DragEvent, UIEvent, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Alert, Button, Drawer, Empty, Input, Popconfirm, Progress, Segmented, Spin, Tag, Tooltip, message as toast } from "antd";
import type { TextAreaRef } from "antd/es/input/TextArea";
import { ChevronDown, Copy, File as FileIcon, Paperclip, RotateCcw, Save, Send, Trash2, Upload, Users, X } from "lucide-react";
import { getBootstrap } from "@/api/service";
import { listDevices, removeDevice, saveDeviceName, updateDeviceName } from "@/api/device";
import { listMessages, sendTextMessage } from "@/api/message";
import { cancelTransfer, uploadGroupFile } from "@/api/file";
import DeviceAvatar from "@/components/DeviceAvatar";
import FileCard from "@/components/FileCard";
import { useDeviceIdentity } from "@/hooks/useDeviceIdentity";
import { useLanSocket } from "@/hooks/useLanSocket";
import { getAccessToken, setCurrentDeviceId } from "@/http";
import { useServiceStore, useUnreadStore } from "@/store";
import type { Device, Message } from "@/types/domain";
import { formatTime } from "@/utils/time";
import { createId } from "@/utils/id";
import { copyText } from "@/utils/clipboard";
import { isFileDrag, readDroppedFiles } from "@/utils/fileDrop";
import { formatFileSize } from "@/utils/fileSize";
import { isNearScrollBottom } from "@/utils/scroll";
import { mergeMessages } from "@/utils/groupChat";
import { estimateRemainingSeconds, formatRemainingTime, formatTransferSpeed } from "@/utils/transfer";
import styles from "./index.module.less";
import GroupAccess from "./components/GroupAccess";

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
  const composerRef = useRef<TextAreaRef>(null);
  const sendingRef = useRef(false);
  const messageListRef = useRef<HTMLDivElement>(null);
  const shouldStickToBottomRef = useRef(true);
  const uploadControllersRef = useRef(new Map<string, AbortController>());
  const uploadSamplesRef = useRef(new Map<string, { time: number; bytes: number; speed: number }>());
  const messagesRef = useRef<Message[]>([]);
  const latestCursor = useRef<string | undefined>(undefined);
  const refreshPending = useRef(false);
  const refreshRequested = useRef(false);
  const historyPending = useRef(false);
  const scrollAnchor = useRef<{ height: number; top: number } | undefined>(undefined);
  const clientId = useDeviceIdentity();
  const serviceRunning = useServiceStore((state) => state.running);
  const configureUnread = useUnreadStore((state) => state.configure);
  const syncUnread = useUnreadStore((state) => state.sync);
  const setVisible = useUnreadStore((state) => state.setVisible);
  const [currentDevice, setCurrentDevice] = useState<Device>();
  const [devices, setDevices] = useState<Device[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [nickname, setNickname] = useState(() => localStorage.getItem("tong-net-device-name") ?? defaultNickname());
  const [membersOpen, setMembersOpen] = useState(false);
  const [memberFilter, setMemberFilter] = useState<"online" | "all">("online");
  const [loading, setLoading] = useState(true);
  const [fatalError, setFatalError] = useState("");
  const [uploads, setUploads] = useState<UploadItem[]>([]);
  const [pendingFiles, setPendingFiles] = useState<{ id: string; file: File }[]>([]);
  const [newMessageCount, setNewMessageCount] = useState(0);
  const [hasHistory, setHasHistory] = useState(false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [sending, setSending] = useState(false);
  const [draggingFiles, setDraggingFiles] = useState(false);
  const dragDepth = useRef(0);

  useEffect(() => {
    const reset = () => { dragDepth.current = 0; setDraggingFiles(false); };
    const preventFileNavigation = (event: globalThis.DragEvent) => {
      if (event.dataTransfer && isFileDrag(event.dataTransfer)) event.preventDefault();
      if (event.type === "drop") reset();
    };
    window.addEventListener("dragover", preventFileNavigation);
    window.addEventListener("drop", preventFileNavigation);
    window.addEventListener("dragend", reset);
    window.addEventListener("blur", reset);
    return () => {
      window.removeEventListener("dragover", preventFileNavigation);
      window.removeEventListener("drop", preventFileNavigation);
      window.removeEventListener("dragend", reset);
      window.removeEventListener("blur", reset);
    };
  }, []);

  const acceptMessages = useCallback((incoming: Message[], prepend = false) => {
    const next = mergeMessages(messagesRef.current, incoming, prepend);
    messagesRef.current = next;
    setMessages(next);
  }, []);

  const refresh = useCallback(async () => {
    if (!currentDevice) return;
    refreshRequested.current = true;
    if (refreshPending.current) return;
    refreshPending.current = true;
    try {
      while (refreshRequested.current) {
        refreshRequested.current = false;
        setDevices((await listDevices()).data);
        const cursor = latestCursor.current;
        const nextMessages = (await listMessages(cursor ? { after: cursor } : undefined)).data;
        const knownIds = new Set(messagesRef.current.map((item) => item.id));
        const incomingCount = nextMessages.filter((item) => !knownIds.has(item.id) && item.fromDeviceId !== currentDevice.id && item.type !== "system").length;
        if (cursor && incomingCount && !shouldStickToBottomRef.current) setNewMessageCount((count) => count + incomingCount);
        if (!cursor) setHasHistory(nextMessages.length === 50);
        latestCursor.current = nextMessages[nextMessages.length - 1]?.id ?? cursor ?? "0";
        acceptMessages(nextMessages);
        await syncUnread();
        if (cursor && nextMessages.length === 50) refreshRequested.current = true;
      }
    } finally { refreshPending.current = false; }
  }, [currentDevice, acceptMessages, syncUnread]);

  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setFatalError("");
    setCurrentDevice(undefined);
    latestCursor.current = undefined;
    messagesRef.current = [];
    setMessages([]);
    shouldStickToBottomRef.current = true;
    const initialize = async () => {
      if (hostMode && !serviceRunning) throw new Error("请先开启互通服务，再进入互通群聊。");
      if (!hostMode) saveDeviceName(nickname);
      const device = hostMode
        ? (await listDevices()).data.find((item) => item.id === "host")
        : (await getBootstrap()).data.currentDevice;
      if (!device) throw new Error("未找到本机主机");
      if (disposed) return;
      setCurrentDeviceId(device.id);
      configureUnread(device.id);
      setNickname(device.name);
      setCurrentDevice(device);
    };
    void initialize().catch((error) => {
      if (!disposed) setFatalError(error.response?.data?.message ?? (hostMode ? error.message : getAccessToken() ? "无法连接同网互通主机" : "无法进入，请检查地址和访问令牌。"));
    }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [clientId, hostMode, serviceRunning, configureUnread]);

  useEffect(() => {
    if (!currentDevice) return;
    void refresh().catch(() => api.error("群聊加载失败，请检查连接"));
    const update = () => {
      setVisible(document.visibilityState === "visible" && document.hasFocus());
      if (document.visibilityState === "visible") void refresh().catch(() => undefined);
    };
    update();
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      setVisible(false);
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [currentDevice, refresh, setVisible]);

  const loadHistory = async () => {
    const container = messageListRef.current;
    const before = messagesRef.current[0]?.id;
    if (!container || !before || !hasHistory || historyPending.current) return;
    historyPending.current = true;
    setHistoryLoading(true);
    try {
      const { data } = await listMessages({ before });
      scrollAnchor.current = { height: container.scrollHeight, top: container.scrollTop };
      acceptMessages(data, true);
      setHasHistory(data.length === 50);
    } catch { api.error("历史消息加载失败"); }
    finally { historyPending.current = false; setHistoryLoading(false); }
  };

  useLayoutEffect(() => {
    const container = messageListRef.current;
    if (!container) return;
    if (scrollAnchor.current) {
      container.scrollTop = scrollAnchor.current.top + container.scrollHeight - scrollAnchor.current.height;
      scrollAnchor.current = undefined;
    } else if (shouldStickToBottomRef.current) container.scrollTop = container.scrollHeight;
  }, [messages, uploads, pendingFiles, loading, historyLoading]);

  const { connected } = useLanSocket(Boolean(currentDevice), currentDevice?.id ?? "", () => { void refresh().catch(() => undefined); });
  const deviceNameMap = useMemo(() => new Map(devices.map((device) => [device.id, device.name])), [devices]);
  const onlineCount = devices.filter((device) => device.status === "online").length;
  const visibleMembers = devices.filter((device) => memberFilter === "all" || device.status === "online");

  const trackMessageScroll = (event: UIEvent<HTMLDivElement>) => {
    const container = event.currentTarget;
    shouldStickToBottomRef.current = isNearScrollBottom(container);
    if (shouldStickToBottomRef.current) setNewMessageCount(0);
    if (container.scrollTop < 40) void loadHistory();
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
    if ((!content && pendingFiles.length === 0) || !currentDevice || sendingRef.current) return;
    sendingRef.current = true;
    const files = pendingFiles;
    followOwnMessage();
    setDraft("");
    setPendingFiles([]);
    setSending(true);
    files.forEach(({ file }) => startUpload(file));
    try {
      if (content) {
        acceptMessages([(await sendTextMessage(content)).data]);
        void refresh().catch(() => undefined);
      }
    } catch (error: any) {
      setDraft((value) => value || content);
      api.error(error.response?.data?.message ?? "消息发送失败");
    } finally { sendingRef.current = false; setSending(false); }
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
    if (!currentDevice) return;
    const id = createId();
    const controller = new AbortController();
    uploadControllersRef.current.set(id, controller);
    uploadSamplesRef.current.set(id, { time: performance.now(), bytes: 0, speed: 0 });
    setUploads((items) => [...items, { id, file, name: file.name, progress: 0, speed: 0, remaining: 0, status: "running" }]);
    const formData = new FormData();
    formData.append("file", file);
    void uploadGroupFile(formData, {
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
      acceptMessages([data]);
      void refresh().catch(() => undefined);
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

  const queueFiles = (files: File[]) => {
    if (!currentDevice || files.length === 0) return;
    setPendingFiles((items) => [...items, ...files.map((file) => ({ id: createId(), file }))]);
    composerRef.current?.focus({ preventScroll: true });
  };

  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    queueFiles(files);
  };

  const handleDragEnter = (event: DragEvent<HTMLElement>) => {
    if (!isFileDrag(event.dataTransfer)) return;
    event.preventDefault();
    dragDepth.current += 1;
    if (currentDevice) setDraggingFiles(true);
  };

  const handleDragLeave = (event: DragEvent<HTMLElement>) => {
    if (!isFileDrag(event.dataTransfer)) return;
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (!dragDepth.current) setDraggingFiles(false);
  };

  const handleDrop = (event: DragEvent<HTMLElement>) => {
    if (!isFileDrag(event.dataTransfer)) return;
    event.preventDefault();
    dragDepth.current = 0;
    setDraggingFiles(false);
    const { files, hasDirectories } = readDroppedFiles(event.dataTransfer);
    if (hasDirectories) api.warning("暂不支持上传文件夹，请选择文件或压缩后上传");
    queueFiles(files);
  };

  const copyMessage = async (content: string) => {
    try { await copyText(content); api.success("已复制"); }
    catch { api.error("复制失败，请长按或选中文字复制"); }
  };

  const removeMember = async (id: string) => {
    try { await removeDevice(id); await refresh(); }
    catch (error: any) { api.error(error.response?.data?.message ?? "移除失败"); }
  };

  if (loading) return <div className={styles.centerState}><Spin size="large" /></div>;
  if (fatalError) return <div className={styles.centerState}><Alert type="error" showIcon title="无法进入" description={fatalError} /></div>;

  return (
    <div className={`${styles.page} ${hostMode ? styles.desktopEmbedded : ""}`}>
      {contextHolder}
      <section className={styles.chat}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDragOver={(event) => {
          if (!isFileDrag(event.dataTransfer)) return;
          event.preventDefault();
          event.dataTransfer.dropEffect = currentDevice ? "copy" : "none";
        }}
        onDrop={handleDrop}
      >
        {draggingFiles && <div className={styles.dropOverlay} data-testid="file-drop-overlay" aria-label="上传文件"><Upload size={40} /></div>}
        <header className={styles.chatHeader}>
          <div className={styles.brand}>
            <img src="/brand/tong-net-logo.png" alt="同网互通" />
            <h1>互通群聊</h1>
          </div>
          {hostMode && serviceRunning && <GroupAccess />}
          <div className={styles.headerActions}>
            <Tag color={connected ? "green" : "orange"}>{connected ? "已连接" : "重连中"}</Tag>
            <Tooltip title="群成员"><Button aria-label="群成员" icon={<Users size={17} />} onClick={() => { setMemberFilter("online"); setMembersOpen(true); }}>{onlineCount} 在线</Button></Tooltip>
          </div>
        </header>
          <div ref={messageListRef} data-testid="message-list" className={styles.messageList} onScroll={trackMessageScroll}>
            {hasHistory && <Button type="text" loading={historyLoading} onClick={loadHistory}>更早的消息</Button>}
            {messages.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="还没有消息" />}
            {messages.map((item) => {
              const mine = item.fromDeviceId === currentDevice?.id;
              return <div key={item.id} className={`${styles.messageRow} ${mine ? styles.mine : ""}`}>
                <div className={styles.messageMeta}>{deviceNameMap.get(item.fromDeviceId) ?? "已移除访问端"} · {formatTime(item.createdAt)}</div>
                <div className={styles.messageBody}>
                  <div className={item.type === "system" ? styles.systemBubble : styles.bubble}>{item.file ? <FileCard file={item.file} hostMode={hostMode} /> : item.content}</div>
                  {item.type === "text" && !item.file && <Tooltip title="复制消息">
                    <Button className={styles.copyMessage} type="text" aria-label="复制消息" icon={<Copy size={15} />} onClick={() => void copyMessage(item.content)} />
                  </Tooltip>}
                </div>
              </div>;
            })}
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
            {pendingFiles.length > 0 && <div className={styles.pendingFiles} role="region" aria-label="待发送文件">
              {pendingFiles.map(({ id, file }) => <div key={id} className={styles.pendingFile}>
                <FileIcon size={20} aria-hidden="true" />
                <div className={styles.pendingFileInfo}><span title={file.name}>{file.name}</span><small>{formatFileSize(file.size)}</small></div>
                <Tooltip title="移除文件"><Button type="text" aria-label={`移除待发送文件 ${file.name}`} icon={<X size={16} />} onClick={() => setPendingFiles((items) => items.filter((item) => item.id !== id))} /></Tooltip>
              </div>)}
            </div>}
            <input ref={fileInputRef} type="file" multiple className={styles.fileInput} onChange={handleFileChange} />
            <Button aria-label="选择文件" icon={<Paperclip size={16} />} onClick={() => fileInputRef.current?.click()} />
            <Input.TextArea ref={composerRef} value={draft} onChange={(event) => setDraft(event.target.value)} onPressEnter={(event) => { if (!event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); if (!event.repeat) void sendMessage(); } }} autoSize={{ minRows: 1, maxRows: 3 }} placeholder="发送到互通群聊" />
            <Button type="primary" disabled={!draft.trim() && pendingFiles.length === 0} loading={sending} icon={<Send size={16} />} onClick={sendMessage}>发送</Button>
          </footer>

      </section>
      <Drawer title="群成员" open={membersOpen} onClose={() => setMembersOpen(false)} size={360}>
        {currentDevice && <div className={styles.profile}>
          <DeviceAvatar device={currentDevice} />
          <div className={styles.profileBody}>
            <label htmlFor="group-nickname">我的名称</label>
            <Input id="group-nickname" value={nickname} maxLength={40} onChange={(event) => setNickname(event.target.value)} onPressEnter={changeNickname} />
          </div>
          <Tooltip title="保存名称"><Button aria-label="保存名称" icon={<Save size={16} />} onClick={changeNickname} /></Tooltip>
        </div>}
        <Segmented
          className={styles.memberFilter}
          block
          value={memberFilter}
          onChange={setMemberFilter}
          options={[
            { label: `在线 (${onlineCount})`, value: "online" },
            { label: `全部 (${devices.length})`, value: "all" },
          ]}
        />
        <div className={styles.memberList}>
          {visibleMembers.length === 0 && <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={memberFilter === "online" ? "暂无在线成员" : "暂无成员"} />}
          {visibleMembers.map((device) => <div key={device.id} className={styles.member}>
            <DeviceAvatar device={device} size="small" />
            <div className={styles.memberName}><strong>{device.name}{device.id === currentDevice?.id ? "（我）" : ""}</strong><small>{device.browserSource}</small></div>
            <Tag color={device.status === "online" ? "green" : "default"}>{device.status === "online" ? "在线" : "离线"}</Tag>
            {hostMode && device.kind === "browser" && device.status === "offline" && <Popconfirm title="移除这个访问端？" description="聊天与文件记录仍会保留。" onConfirm={() => removeMember(device.id)} okText="移除" cancelText="取消"><Tooltip title="移除访问端"><Button danger type="text" aria-label={`移除访问端 ${device.name}`} icon={<Trash2 size={16} />} /></Tooltip></Popconfirm>}
          </div>)}
        </div>
      </Drawer>
    </div>
  );
}
