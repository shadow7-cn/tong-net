import { useCallback, useEffect, useMemo, useState } from "react";
import { Alert, Button, Table, Tabs, Tag, message as toast } from "antd";
import type { ColumnsType } from "antd/es/table";
import { RefreshCw } from "lucide-react";
import { request } from "@/http";
import { useServiceStore } from "@/store";
import type { Device, FileRecord, Message, TransferTask } from "@/types/domain";
import { formatFileSize } from "@/utils/fileSize";
import { formatDateTime } from "@/utils/time";
import styles from "./index.module.less";

type RecordsData = { devices: Device[]; messages: Message[]; files: FileRecord[]; transfers: TransferTask[] };
const empty: RecordsData = { devices: [], messages: [], files: [], transfers: [] };
const tableScroll = { x: "max-content" as const, y: "calc(100vh - 360px)" };
const tablePagination = { pageSize: 20, showSizeChanger: false };

export default function Records() {
  const [api, contextHolder] = toast.useMessage();
  const running = useServiceStore((state) => state.running);
  const [data, setData] = useState(empty);
  const [loading, setLoading] = useState(false);
  const load = useCallback(async () => {
    if (!running) return;
    setLoading(true);
    try { setData((await request.get<RecordsData>("/records")).data); }
    catch (error: any) { api.error(error.response?.data?.message ?? "记录加载失败"); }
    finally { setLoading(false); }
  }, [running]);
  useEffect(() => { void load(); }, [load]);
  const names = useMemo(() => new Map(data.devices.map((device) => [device.id, device.name])), [data.devices]);

  const deviceColumns: ColumnsType<Device> = [
    { title: "访问端名称", dataIndex: "name" },
    { title: "来源", dataIndex: "browserSource" },
    { title: "类型", dataIndex: "kind", render: (kind) => kind === "host" ? "本机主机" : "浏览器访问端" },
    { title: "状态", dataIndex: "status", render: (status) => <Tag color={status === "online" ? "green" : "default"}>{status === "online" ? "在线" : "离线"}</Tag> },
    { title: "记录状态", dataIndex: "removed", render: (removed) => removed ? <Tag>已移除</Tag> : <Tag color="blue">有效</Tag> },
    { title: "最后出现", dataIndex: "lastSeenAt", render: formatDateTime },
  ];
  const messageColumns: ColumnsType<Message> = [
    { title: "发送访问端", dataIndex: "fromDeviceId", render: (id) => names.get(id) ?? id },
    { title: "接收访问端", dataIndex: "toDeviceId", render: (id) => names.get(id) ?? id },
    { title: "类型", dataIndex: "type", width: 100, render: (type) => type === "text" ? "文字" : type === "file" ? "文件" : "系统" },
    { title: "内容", dataIndex: "content", ellipsis: true },
    { title: "时间", dataIndex: "createdAt", render: formatDateTime },
  ];
  const fileColumns: ColumnsType<FileRecord> = [
    { title: "文件名", dataIndex: "name" },
    { title: "大小", dataIndex: "size", render: formatFileSize },
    { title: "状态", dataIndex: "status", render: (status) => <Tag color={status === "available" ? "green" : "red"}>{status === "available" ? "可用" : "失败"}</Tag> },
    { title: "时间", dataIndex: "createdAt", render: formatDateTime },
  ];
  const transferColumns: ColumnsType<TransferTask> = [
    { title: "文件名", dataIndex: "fileName" },
    { title: "方向", dataIndex: "kind", width: 100, render: (kind) => kind === "upload" ? "上传" : "下载" },
    { title: "对端访问端", dataIndex: "peerName" },
    { title: "状态", dataIndex: "status", render: (status) => <Tag color={status === "failed" ? "red" : status === "success" ? "green" : "blue"}>{status === "failed" ? "失败" : status === "success" ? "完成" : "传输中"}</Tag> },
    { title: "时间", dataIndex: "createdAt", render: (value) => value ? formatDateTime(value) : "--" },
  ];

  return <div className={styles.page}>
    {contextHolder}
    <header className={styles.header}><div><h1>本地记录</h1><p>访问端、聊天、文件和传输记录保存在本机 SQLite。</p></div><Button icon={<RefreshCw size={16} />} disabled={!running} loading={loading} onClick={load}>刷新</Button></header>
    {!running && <Alert type="info" showIcon message="开启互通服务后可查看本地记录。" />}
    <Tabs className={styles.tabs} items={[
      { key: "devices", label: `访问端 (${data.devices.length})`, children: <div className={styles.tableScroll}><Table scroll={tableScroll} pagination={tablePagination} rowKey="id" columns={deviceColumns} dataSource={data.devices} loading={loading} /></div> },
      { key: "messages", label: `聊天 (${data.messages.length})`, children: <div className={styles.tableScroll}><Table scroll={tableScroll} pagination={tablePagination} rowKey="id" columns={messageColumns} dataSource={data.messages} loading={loading} /></div> },
      { key: "files", label: `文件 (${data.files.length})`, children: <div className={styles.tableScroll}><Table scroll={tableScroll} pagination={tablePagination} rowKey="id" columns={fileColumns} dataSource={data.files} loading={loading} /></div> },
      { key: "transfers", label: `传输 (${data.transfers.length})`, children: <div className={styles.tableScroll}><Table scroll={tableScroll} pagination={tablePagination} rowKey="id" columns={transferColumns} dataSource={data.transfers} loading={loading} /></div> },
    ]} />
  </div>;
}
