import { useEffect, useMemo } from "react";
import { Button, Empty, Form, Input, Space, Table, Tabs, Tag, message } from "antd";
import { CircleStop, Network, Play, RefreshCw, Save } from "lucide-react";
import {
  getEasyTierConfig,
  saveEasyTierConfig,
  type EasyTierConfig,
  type EasyTierMember,
} from "@/api/easytier";
import { useEasyTierStore } from "@/store";
import styles from "./index.module.less";

const initialValues: EasyTierConfig = {
  networkName: "",
  networkSecret: "",
  deviceName: `同网互通-${navigator.platform || "桌面设备"}`,
  serverAddress: "",
};

const memberColumns = [
  {
    title: "设备",
    dataIndex: "hostname",
    key: "hostname",
    render: (value: string, member: EasyTierMember) => (
      <Space size={6}>
        <span>{value || "未命名设备"}</span>
        {member.local && <Tag color="blue">本机</Tag>}
      </Space>
    ),
  },
  {
    title: "虚拟 IP",
    dataIndex: "ipv4",
    key: "ipv4",
    render: (value: string) => value || "--",
  },
  {
    title: "延迟",
    dataIndex: "latency",
    key: "latency",
    width: 90,
    render: (value?: string) => !value || value === "-" ? "--" : `${value} ms`,
  },
  {
    title: "协议",
    dataIndex: "protocol",
    key: "protocol",
    width: 90,
    render: (value?: string) => !value || value === "-" ? "--" : value.toUpperCase(),
  },
  {
    title: "收 / 发",
    key: "traffic",
    width: 150,
    render: (_: unknown, member: EasyTierMember) =>
      `${member.rxBytes || "--"} / ${member.txBytes || "--"}`,
  },
];

export default function VirtualLan() {
  const [form] = Form.useForm<EasyTierConfig>();
  const [api, contextHolder] = message.useMessage();
  const status = useEasyTierStore();

  useEffect(() => {
    void status.refresh();
    void getEasyTierConfig()
      .then((config) => {
        if (config.networkName || config.serverAddress) form.setFieldsValue(config);
      })
      .catch((error) => api.error(`读取虚拟局域网配置失败：${String(error)}`));
    const timer = window.setInterval(() => void status.refresh().catch(() => undefined), 1500);
    return () => window.clearInterval(timer);
  }, [form, status.refresh]);

  const connect = async (values: EasyTierConfig) => {
    try {
      await status.connect(values);
      api.success("已启动内置 EasyTier Core，正在获取虚拟 IP");
    } catch (error) {
      api.error(String(error));
    }
  };

  const saveConfig = async () => {
    try {
      const values = await form.validateFields();
      await saveEasyTierConfig(values);
      api.success("虚拟局域网配置已加密保存");
    } catch (error) {
      if (error instanceof Error) api.error(String(error));
    }
  };

  const disconnect = async () => {
    try {
      await status.disconnect();
      api.success("已断开虚拟局域网");
    } catch (error) {
      api.error(String(error));
    }
  };

  const logText = useMemo(
    () => status.logs.length ? status.logs.join("\n") : "暂无诊断日志",
    [status.logs],
  );

  return (
    <div className={styles.page}>
      {contextHolder}
      <header className={styles.header}>
        <div>
          <Space align="center">
            <Network size={28} />
            <h1>虚拟局域网</h1>
          </Space>
          <p>通过内置 EasyTier Core 加入远程网络，并查看当前在线成员。</p>
        </div>
        <Space>
          <Tag color={status.connected ? "green" : status.running ? "processing" : "default"}>
            {status.phase}
          </Tag>
          <Button
            icon={<RefreshCw size={16} />}
            onClick={() => void status.refresh()}
            title="刷新状态"
          />
        </Space>
      </header>

      <div className={styles.summary}>
        <div><span>当前网络</span><strong>{status.networkName || "--"}</strong></div>
        <div><span>本机设备</span><strong>{status.deviceName || "--"}</strong></div>
        <div><span>虚拟 IP</span><strong>{status.virtualIp || "--"}</strong></div>
        <div><span>网络成员</span><strong>{status.members.length}</strong></div>
      </div>

      <div className={styles.workspace}>
        <section className={styles.formPane}>
          <Form form={form} layout="vertical" initialValues={initialValues} onFinish={connect}>
            <Form.Item label="网络名称" name="networkName" rules={[{ required: true, message: "请输入网络名称" }]}>
              <Input disabled={status.running} placeholder="例如：我的虚拟局域网" />
            </Form.Item>
            <Form.Item label="网络密码" name="networkSecret" rules={[{ required: !status.running, message: "请输入网络密码" }]}>
              <Input.Password
                disabled={status.running}
                placeholder={status.running ? "运行期间不可修改密码" : "输入网络密码"}
                autoComplete="off"
              />
            </Form.Item>
            <Form.Item label="本机设备名称" name="deviceName" rules={[{ required: true, message: "请输入设备名称" }]}>
              <Input disabled={status.running} />
            </Form.Item>
            <Form.Item
              label="连接地址"
              name="serverAddress"
              extra="填写一个 IP:端口，同网互通会同时通过 TCP 和 UDP 连接。"
              rules={[
                { required: true, message: "请输入连接地址" },
                {
                  pattern: /^(?:\d{1,3}\.){3}\d{1,3}:\d{1,5}$/,
                  message: "请输入 IP:端口，例如 203.0.113.10:11010",
                },
              ]}
            >
              <Input disabled={status.running} placeholder="203.0.113.10:11010" />
            </Form.Item>
            {status.running ? (
              <Button danger block icon={<CircleStop size={16} />} loading={status.loading} onClick={() => void disconnect()}>
                断开虚拟局域网
              </Button>
            ) : (
              <Space.Compact block>
                <Button icon={<Save size={16} />} onClick={() => void saveConfig()}>
                  保存配置
                </Button>
                <Button
                  className={styles.connectButton}
                  type="primary"
                  htmlType="submit"
                  icon={<Play size={16} />}
                  loading={status.loading}
                >
                  连接虚拟局域网
                </Button>
              </Space.Compact>
            )}
          </Form>
        </section>

        <section className={styles.contentPane}>
          <Tabs
            className={styles.tabs}
            items={[
              {
                key: "members",
                label: `网络成员 (${status.members.length})`,
                children: status.members.length ? (
                  <Table<EasyTierMember>
                    rowKey="id"
                    columns={memberColumns}
                    dataSource={status.members}
                    pagination={false}
                    size="middle"
                    scroll={{ x: 650, y: 350 }}
                  />
                ) : (
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={status.running ? "正在等待成员信息" : "连接后显示虚拟局域网成员"}
                  />
                ),
              },
              {
                key: "diagnostics",
                label: "诊断信息",
                children: <pre className={styles.logs}>{logText}</pre>,
              },
            ]}
          />
        </section>
      </div>
    </div>
  );
}
