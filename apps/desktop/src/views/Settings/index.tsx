import { useEffect, useState } from "react";
import { Alert, Button, Form, Input, InputNumber, Switch, message } from "antd";
import { getSettings, updateSettings } from "@/api/service";
import type { AppSettings } from "@/types/domain";
import { useServiceStore } from "@/store";
import styles from "./index.module.less";

export default function Settings() {
  const [api, contextHolder] = message.useMessage();
  const [form] = Form.useForm<AppSettings>();
  const [loading, setLoading] = useState(true);
  const running = useServiceStore((state) => state.running);
  const allowTokenlessAccess = Form.useWatch("allowTokenlessAccess", form);

  useEffect(() => {
    getSettings().then((settings) => form.setFieldsValue(settings)).catch((error) => api.error(String(error))).finally(() => setLoading(false));
  }, [form]);

  const save = async (values: AppSettings) => {
    try { await updateSettings(values); api.success("设置已保存"); }
    catch (error) { api.error(String(error)); }
  };

  return <div className={styles.page}>
    {contextHolder}
    <header className={styles.header}><h1>设置</h1><p>服务端口、主机名称和文件目录保存在本机。</p></header>
    {running && <Alert type="info" showIcon message="互通服务运行中，目前只能修改“打开软件时自动开启互通”。" />}
    <Form form={form} layout="vertical" disabled={loading} onFinish={save}>
      <Form.Item
        label="打开软件时自动开启互通"
        name="autoStartService"
        valuePropName="checked"
        extra="关闭后，下次打开软件需要手动点击“开启互通”。"
      ><Switch /></Form.Item>
      <Form.Item label="本机主机名称" name="hostName" rules={[{ required: true, message: "请输入本机主机名称" }, { max: 40 }]}><Input disabled={running} /></Form.Item>
      <Form.Item label="服务端口" name="port" rules={[{ required: true }]}><InputNumber disabled={running} min={1024} max={65535} style={{ width: 180 }} /></Form.Item>
      <Form.Item label="文件保存目录" name="saveDir" rules={[{ required: true, message: "请输入保存目录" }]}><Input disabled={running} /></Form.Item>
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
      <Form.Item label="启动时清理临时文件" name="cleanupTemp" valuePropName="checked"><Switch disabled={running} /></Form.Item>
      <Button type="primary" htmlType="submit" loading={loading}>保存设置</Button>
    </Form>
  </div>;
}
