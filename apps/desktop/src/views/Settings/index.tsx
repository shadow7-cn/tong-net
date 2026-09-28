import { useEffect, useState } from "react";
import { Alert, Button, Form, Input, InputNumber, Switch, Tabs, Tooltip, message } from "antd";
import { getSettings, updateSettings, isTauri, openSaveDirectory } from "@/api/service";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FolderSearch, RefreshCw } from "lucide-react";
import { useUpdateStore } from "@/store/update";
import type { AppSettings } from "@/types/domain";
import { useServiceStore } from "@/store";
import styles from "./index.module.less";

export default function Settings() {
  const [api, contextHolder] = message.useMessage();
  const [form] = Form.useForm<AppSettings>();
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [tab, setTab] = useState("general");
  const running = useServiceStore((state) => state.running);
  const { currentVersion, checking, check } = useUpdateStore();
  const allowTokenlessAccess = Form.useWatch("allowTokenlessAccess", form);

  useEffect(() => {
    getSettings().then((settings) => form.setFieldsValue(settings)).catch((error) => api.error(String(error))).finally(() => setLoading(false));
  }, [form]);

  const save = async (values: AppSettings) => {
    setSaving(true);
    try { await updateSettings(values); api.success("设置已保存"); }
    catch (error) { api.error(String(error)); }
    finally { setSaving(false); }
  };

  const chooseDirectory = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "选择文件保存目录", defaultPath: form.getFieldValue("saveDir") || undefined });
      if (typeof selected === "string") form.setFieldValue("saveDir", selected);
    } catch (error) { api.error(String(error)); }
  };

  return <div className={styles.page}>
    {contextHolder}
    <header className={styles.header}><h1>设置</h1><p>服务端口、主机名称和文件目录保存在本机。</p></header>
    {running && <Alert type="info" showIcon message="互通服务运行中，目前只能修改“打开软件时自动开启互通”。" />}
    <Form form={form} layout="vertical" disabled={loading} onFinish={save} onFinishFailed={({ errorFields }) => {
      const field = errorFields[0]?.name[0];
      setTab(field === "saveDir" ? "files" : "general");
    }}>
      <Tabs activeKey={tab} onChange={setTab} items={[
        { key: "general", label: "常规", forceRender: true, children: <>
      <Form.Item
        label="打开软件时自动开启互通"
        name="autoStartService"
        valuePropName="checked"
        extra="关闭后，下次打开软件需要手动点击“开启互通”。"
      ><Switch /></Form.Item>
      <Form.Item label="本机主机名称" name="hostName" rules={[{ required: true, message: "请输入本机主机名称" }, { max: 40 }]}><Input disabled={running} /></Form.Item>
      <Form.Item label="服务端口" name="port" rules={[{ required: true }]}><InputNumber disabled={running} min={1024} max={65535} style={{ width: 180 }} /></Form.Item>
        </> },
        { key: "files", label: "文件", forceRender: true, children: <>
      <Form.Item label="文件保存目录" extra="修改后，历史文件将从新目录查找；未迁移的文件会显示为不存在。">
        <div className={styles.directory}>
          <Form.Item name="saveDir" noStyle rules={[{ required: true, message: "请输入保存目录" }]}><Input disabled={running} aria-label="文件保存目录" /></Form.Item>
          <Tooltip title="选择文件夹"><Button aria-label="选择文件夹" disabled={running || !isTauri()} icon={<FolderSearch size={18} />} onClick={chooseDirectory} /></Tooltip>
          <Tooltip title="打开已保存的目录"><Button aria-label="打开已保存的目录" disabled={!isTauri()} icon={<FolderOpen size={18} />} onClick={async () => {
            try { await openSaveDirectory(); } catch (error) { api.error(String(error)); }
          }} /></Tooltip>
        </div>
      </Form.Item>
      <Form.Item label="启动时清理临时文件" tooltip="启动互通服务时，清理上传中断或异常退出后残留的未完成文件（.part），释放磁盘空间。不会删除已传输完成的文件、聊天记录或传输记录。" name="cleanupTemp" valuePropName="checked"><Switch disabled={running} /></Form.Item>
        </> },
        { key: "security", label: "连接安全", forceRender: true, children: <>
      <Form.Item
        label="允许无令牌访问"
        name="allowTokenlessAccess"
        valuePropName="checked"
        extra="开启后，同一局域网中的任何访问端都可以直接连接。仅建议在可信网络中临时使用。"
      ><Switch disabled={running} /></Form.Item>
      <Form.Item
        label="每次开启服务生成新令牌"
        name="rotateToken"
        valuePropName="checked"
        extra={allowTokenlessAccess ? "无令牌访问开启时，此选项暂不生效。" : undefined}
      ><Switch disabled={running || allowTokenlessAccess} /></Form.Item>
        </> },
        ...(isTauri() ? [{ key: "updates", label: "软件更新", children: <section className={styles.updates}>
      <div><h2>软件更新</h2><span>当前版本：{currentVersion ? `v${currentVersion}` : "读取中"}</span></div>
      <Button icon={<RefreshCw size={16} />} loading={checking} onClick={async () => {
        try {
          const result = await check();
          if (!result.available) api.success("当前已是最新版本");
        } catch (error) { api.error(String(error)); }
      }}>检查更新</Button>
    </section> }] : []),
      ]} />
      {tab !== "updates" && <Button type="primary" htmlType="submit" loading={saving} disabled={loading}>保存设置</Button>}
    </Form>
  </div>;
}
