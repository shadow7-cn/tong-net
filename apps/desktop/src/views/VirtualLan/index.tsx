import { useEffect, useMemo } from "react";
import { Alert, Button, Empty, Form, Input, Modal, Space, Table, Tabs, Tag, message } from "antd";
import { CircleStop, Network, Play, RefreshCw, Save, ShieldAlert } from "lucide-react";
import {
  getEasyTierConfig,
  saveEasyTierConfig,
  type EasyTierConfig,
  type EasyTierMember,
} from "@/api/easytier";
import { useEasyTierStore } from "@/store";
import { continueInsecureConnection, getEasyTierMemberRole } from "@/utils/easytier";
import styles from "./index.module.less";

const initialValues: EasyTierConfig = {
  serverUrl: "",
  networkName: "",
  networkPassword: "",
  deviceName: `同网互通-${navigator.platform || "桌面设备"}`,
  allowInsecureHttp: false,
};

const memberColumns = [
  {
    title: "节点",
    dataIndex: "hostname",
    key: "hostname",
    render: (value: string, member: EasyTierMember) => {
      const role = getEasyTierMemberRole(member);
      return (
        <Space size={6}>
          <span>{value || "未命名节点"}</span>
          {role === "local" && <Tag color="blue">本机</Tag>}
          {role === "shared" && <Tag color="green">共享节点</Tag>}
          {role === "service" && <Tag>网络服务</Tag>}
        </Space>
      );
    },
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
  const [modal, modalContextHolder] = Modal.useModal();
  const status = useEasyTierStore();
  const serverUrl = Form.useWatch("serverUrl", form) ?? "";

  useEffect(() => {
    void status.refresh();
    void getEasyTierConfig()
      .then((config) => {
        if (config.networkName || config.serverUrl) form.setFieldsValue(config);
      })
      .catch((error) => api.error(`读取虚拟局域网配置失败：${String(error)}`));
    const timer = window.setInterval(() => void status.refresh().catch(() => undefined), 1500);
    return () => window.clearInterval(timer);
  }, [form, status.refresh]);

  const performConnect = async (values: EasyTierConfig) => {
    try {
      await status.connect(values);
      api.success("已启动内置 EasyTier Core，正在获取虚拟 IP");
    } catch (error) {
      api.error(String(error));
    }
  };

  const connect = async (values: EasyTierConfig) => {
    if (values.serverUrl.trim().startsWith("http://") && !values.allowInsecureHttp) {
      modal.confirm({
        title: "确认使用未加密连接",
        icon: <ShieldAlert size={22} color="#d48b19" />,
        content: "HTTP 会以明文传输网络登录信息。仅在你信任的网络或已通过其他方式保护链路时使用。",
        okText: "我了解风险，继续",
        cancelText: "取消",
        onOk: () => continueInsecureConnection(
          values,
          () => form.setFieldValue("allowInsecureHttp", true),
          performConnect,
        ),
      });
      return;
    }
    await performConnect(values);
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
  const deviceMembers = useMemo(
    () => status.members.filter((member) => {
      const role = getEasyTierMemberRole(member);
      return role === "local" || role === "device";
    }),
    [status.members],
  );

  return (
    <div className={styles.page}>
      {contextHolder}
      {modalContextHolder}
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
          {status.serverMode && (
            <Tag color={status.serverMode === "private" ? "blue" : "green"}>
              {status.serverMode === "private" ? "私有节点" : "公共节点"}
            </Tag>
          )}
          {(status.insecureHttp || serverUrl.trim().startsWith("http://")) && (
            <Tag color="warning">未加密</Tag>
          )}
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
        <div><span>在线设备</span><strong>{deviceMembers.length}</strong></div>
      </div>

      <div className={styles.workspace}>
        <section className={styles.formPane}>
          <Form form={form} layout="vertical" initialValues={initialValues} onFinish={connect}>
            <Form.Item
              label="组网服务端地址"
              name="serverUrl"
              extra="填写 Linux 组网服务的完整管理地址。"
              rules={[
                { required: true, message: "请输入组网服务端地址" },
                {
                  pattern: /^https?:\/\/[^/\s]+\/?$/,
                  message: "请输入完整地址，例如 https://vpn.example.com",
                },
              ]}
            >
              <Input disabled={status.running} placeholder="https://vpn.example.com" />
            </Form.Item>
            <Form.Item label="网络名称" name="networkName" rules={[{ required: true, message: "请输入网络名称" }]}>
              <Input disabled={status.running} placeholder="例如：我的虚拟局域网" />
            </Form.Item>
            <Form.Item label="网络密码" name="networkPassword" rules={[{ required: !status.running, message: "请输入网络密码" }]}>
              <Input.Password
                disabled={status.running}
                placeholder={status.running ? "运行期间不可修改密码" : "输入网络密码"}
                autoComplete="off"
              />
            </Form.Item>
            <Form.Item label="本机设备名称" name="deviceName" rules={[{ required: true, message: "请输入设备名称" }]}>
              <Input disabled={status.running} />
            </Form.Item>
            {serverUrl.trim().startsWith("http://") && (
              <Alert
                className={styles.insecureAlert}
                type="warning"
                showIcon
                message="当前服务端使用 HTTP，连接信息不会经过 HTTPS 加密。"
              />
            )}
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
                label: `在线设备 (${deviceMembers.length})`,
                children: deviceMembers.length ? (
                  <Table<EasyTierMember>
                    rowKey="id"
                    columns={memberColumns}
                    dataSource={deviceMembers}
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
