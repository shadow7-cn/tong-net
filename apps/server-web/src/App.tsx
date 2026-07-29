import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Descriptions,
  Drawer,
  Empty,
  Form,
  Input,
  Layout,
  Menu,
  Modal,
  Pagination,
  Popconfirm,
  Segmented,
  Select,
  Space,
  Spin,
  Statistic,
  Table,
  Tag,
  Typography,
} from "antd";
import type { ColumnsType } from "antd/es/table";
import {
  Activity,
  ClipboardList,
  Gauge,
  LogOut,
  Menu as MenuIcon,
  Network as NetworkIcon,
  Plus,
  RadioTower,
  RefreshCw,
  Settings as SettingsIcon,
  ShieldCheck,
  Smartphone,
  WandSparkles,
  X,
} from "lucide-react";
import {
  createHashRouter,
  Navigate,
  Outlet,
  RouterProvider,
  useLocation,
  useNavigate,
} from "react-router-dom";
import { api, type AuditResult, type Member, type Network, type Overview, type PublicInfo, type Settings } from "@/api";

const randomPassword = () => {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
  const values = crypto.getRandomValues(new Uint8Array(20));
  return Array.from(values, (value) => chars[value % chars.length]).join("");
};

const formatTime = (value?: string) =>
  value ? new Intl.DateTimeFormat("zh-CN", { dateStyle: "short", timeStyle: "short" }).format(new Date(value)) : "-";

const formatBytes = (value?: number | string) => {
  const bytes = Number(value ?? 0);
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
};

type AuthContextValue = {
  info: PublicInfo;
  refreshInfo: () => Promise<void>;
  logout: () => Promise<void>;
};

let authContext: AuthContextValue | null = null;
const useAuth = () => {
  if (!authContext) throw new Error("认证上下文尚未初始化");
  return authContext;
};

export default function App() {
  const { message } = AntApp.useApp();
  const [loading, setLoading] = useState(true);
  const [info, setInfo] = useState<PublicInfo>();
  const [authenticated, setAuthenticated] = useState(false);

  const refreshInfo = useCallback(async () => {
    const value = await api.info();
    setInfo(value);
  }, []);

  useEffect(() => {
    Promise.all([api.info(), api.overview().then(() => true).catch(() => false)])
      .then(([value, loggedIn]) => {
        setInfo(value);
        setAuthenticated(loggedIn);
      })
      .catch((error) => message.error(error instanceof Error ? error.message : String(error)))
      .finally(() => setLoading(false));
  }, [message]);

  if (loading || !info) {
    return <div className="boot-screen"><Spin size="large" /></div>;
  }
  if (!info.initialized) {
    return <Setup info={info} onComplete={async () => { await refreshInfo(); setAuthenticated(true); }} />;
  }
  if (!authenticated) {
    return <Login siteName={info.siteName} onSuccess={() => setAuthenticated(true)} />;
  }

  authContext = {
    info,
    refreshInfo,
    logout: async () => {
      await api.logout();
      setAuthenticated(false);
    },
  };
  return <RouterProvider router={router} />;
}

function Setup({ info, onComplete }: { info: PublicInfo; onComplete: () => Promise<void> }) {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm();
  const mode = Form.useWatch("mode", form) ?? "private";
  const [submitting, setSubmitting] = useState(false);

  const submit = async (values: Record<string, string>) => {
    setSubmitting(true);
    try {
      await api.setup(values);
      await api.login({ username: values.adminUsername, password: values.adminPassword });
      message.success("服务端初始化完成");
      await onComplete();
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="auth-page">
      <Card className="auth-panel">
        <Brand subtitle="首次设置" />
        <Typography.Title level={2}>建立你的组网服务</Typography.Title>
        <Typography.Paragraph type="secondary">
          创建管理员并选择节点模式。端口来自 Docker 配置，之后可在设置页查看。
        </Typography.Paragraph>
        <Form
          form={form}
          layout="vertical"
          initialValues={{ mode: "private", siteName: "同网互通服务", publicHost: location.hostname }}
          onFinish={submit}
          requiredMark={false}
        >
          <div className="form-grid">
            <Form.Item label="管理员用户名" name="adminUsername" rules={[{ required: true }, { min: 2 }, { max: 40 }]}>
              <Input autoComplete="username" />
            </Form.Item>
            <PasswordField form={form} name="adminPassword" label="管理员密码" />
            <Form.Item label="站点名称" name="siteName" rules={[{ required: true }, { max: 40 }]}>
              <Input />
            </Form.Item>
            <Form.Item label="对外 IP 或域名" name="publicHost" rules={[{ required: true }]}>
              <Input placeholder="vpn.example.com" />
            </Form.Item>
          </div>
          <Form.Item label="节点模式" name="mode">
            <Segmented
              block
              options={[
                { label: "私有节点", value: "private" },
                { label: "公共节点", value: "public" },
              ]}
            />
          </Form.Item>
          {mode === "private" && (
            <div className="form-grid">
              <Form.Item label="首个网络名称" name="networkName" rules={[{ required: true }, { max: 64 }]}>
                <Input placeholder="家庭网络" />
              </Form.Item>
              <PasswordField form={form} name="networkPassword" label="网络密码" />
            </div>
          )}
          <Alert
            type="info"
            showIcon
            message={mode === "private" ? "私有模式由服务端签发每台设备的独立凭据。" : "公共模式不保存用户填写的网络名称和网络密码。"}
          />
          <Button className="primary-submit" type="primary" htmlType="submit" loading={submitting} block>
            完成设置
          </Button>
        </Form>
        <div className="port-note">Web {info.webPort} · EasyTier TCP/UDP {info.easytierPort}</div>
      </Card>
    </div>
  );
}

function PasswordField({ form, name, label }: { form: ReturnType<typeof Form.useForm>[0]; name: string; label: string }) {
  return (
    <Form.Item label={label} required>
      <Space.Compact block>
        <Form.Item name={name} noStyle rules={[{ required: true }, { min: 8, message: "至少 8 个字符" }]}>
          <Input.Password aria-label={label} autoComplete="new-password" />
        </Form.Item>
        <Button icon={<WandSparkles size={16} />} title="生成随机密码" onClick={() => form.setFieldValue(name, randomPassword())} />
      </Space.Compact>
    </Form.Item>
  );
}

function ResetPasswordFields({
  initialPassword,
  onPasswordChange,
  onAdminPasswordChange,
}: {
  initialPassword: string;
  onPasswordChange: (value: string) => void;
  onAdminPasswordChange: (value: string) => void;
}) {
  const [password, setPassword] = useState(initialPassword);
  const updatePassword = (value: string) => {
    setPassword(value);
    onPasswordChange(value);
  };
  return (
    <Space direction="vertical" className="modal-fields">
      <Alert type="warning" showIcon message="已有设备凭据和会话会立即失效。" />
      <Space.Compact block>
        <Input.Password value={password} onChange={(event) => updatePassword(event.target.value)} />
        <Button
          icon={<WandSparkles size={15} />}
          title="生成随机密码"
          onClick={() => updatePassword(randomPassword())}
        />
      </Space.Compact>
      <Input.Password
        placeholder="管理员密码"
        onChange={(event) => onAdminPasswordChange(event.target.value)}
      />
    </Space>
  );
}

function Login({ siteName, onSuccess }: { siteName: string; onSuccess: () => void }) {
  const { message } = AntApp.useApp();
  const [loading, setLoading] = useState(false);
  return (
    <div className="auth-page">
      <Card className="login-panel">
        <Brand subtitle="组网服务管理" />
        <Typography.Title level={2}>{siteName}</Typography.Title>
        <Typography.Paragraph type="secondary">登录后管理网络、设备和运行状态。</Typography.Paragraph>
        <Form
          layout="vertical"
          requiredMark={false}
          onFinish={async (values) => {
            setLoading(true);
            try {
              await api.login(values);
              onSuccess();
            } catch (error) {
              message.error(error instanceof Error ? error.message : String(error));
            } finally {
              setLoading(false);
            }
          }}
        >
          <Form.Item label="用户名" name="username" rules={[{ required: true }]}>
            <Input autoComplete="username" />
          </Form.Item>
          <Form.Item label="密码" name="password" rules={[{ required: true }]}>
            <Input.Password autoComplete="current-password" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={loading} block>登录</Button>
        </Form>
      </Card>
    </div>
  );
}

const routes = [
  { path: "/overview", label: "概览", icon: <Gauge size={18} /> },
  { path: "/networks", label: "网络", icon: <NetworkIcon size={18} /> },
  { path: "/devices", label: "设备", icon: <Smartphone size={18} /> },
  { path: "/audit", label: "审计", icon: <ClipboardList size={18} /> },
  { path: "/settings", label: "设置", icon: <SettingsIcon size={18} /> },
];

function Shell() {
  const navigate = useNavigate();
  const locationValue = useLocation();
  const { info, logout } = useAuth();
  const [drawer, setDrawer] = useState(false);
  const menu = (
    <Menu
      mode="inline"
      selectedKeys={[locationValue.pathname]}
      items={routes.map((item) => ({ key: item.path, icon: item.icon, label: item.label }))}
      onClick={({ key }) => { navigate(key); setDrawer(false); }}
    />
  );
  return (
    <Layout className="app-shell">
      <Layout.Sider width={236} className="app-sider">
        <Brand subtitle="组网服务控制台" />
        {menu}
        <div className="sider-status">
          <div><RadioTower size={16} /> EasyTier {info.easytierPort}</div>
          <Tag color={info.mode === "private" ? "blue" : "green"}>{info.mode === "private" ? "私有节点" : "公共节点"}</Tag>
        </div>
      </Layout.Sider>
      <Layout>
        <header className="mobile-header">
          <Button type="text" icon={<MenuIcon />} onClick={() => setDrawer(true)} />
          <Brand subtitle="" compact />
        </header>
        <Layout.Content className="app-content"><Outlet /></Layout.Content>
      </Layout>
      <Drawer open={drawer} onClose={() => setDrawer(false)} placement="left" width={280} title={<Brand subtitle="组网服务控制台" compact />}>
        {menu}
        <Button danger type="text" icon={<LogOut size={16} />} onClick={logout}>退出登录</Button>
      </Drawer>
    </Layout>
  );
}

function Brand({ subtitle, compact = false }: { subtitle: string; compact?: boolean }) {
  return (
    <div className={`brand ${compact ? "brand-compact" : ""}`}>
      <img src="/brand/tong-net-logo.png" alt="" />
      <div>
        <strong>同网互通</strong>
        {subtitle && <span>{subtitle}</span>}
      </div>
    </div>
  );
}

function PageHeader({ title, description, action }: { title: string; description: string; action?: React.ReactNode }) {
  return (
    <div className="page-header">
      <div><Typography.Title level={2}>{title}</Typography.Title><Typography.Text type="secondary">{description}</Typography.Text></div>
      {action}
    </div>
  );
}

function OverviewPage() {
  const { message } = AntApp.useApp();
  const [data, setData] = useState<Overview>();
  const load = useCallback(() => api.overview().then(setData).catch((error) => message.error(error.message)), [message]);
  useEffect(() => { void load(); const timer = window.setInterval(load, 10000); return () => clearInterval(timer); }, [load]);
  if (!data) return <Spin />;
  return (
    <div className="page">
      <PageHeader title="运行概览" description="共享节点、网络和在线设备的当前状态。" action={<Button icon={<RefreshCw size={16} />} onClick={load}>刷新</Button>} />
      {!data.easytier.healthy && <Alert type="error" showIcon message="EasyTier 运行异常" description={data.easytier.lastError || "请重试启动或检查容器日志。"} action={<Button onClick={() => api.retryEasyTier().then(load)}>重试</Button>} />}
      <div className="metric-grid">
        <Card><Statistic title="节点状态" value={data.easytier.healthy ? "正常" : "异常"} prefix={<Activity size={20} />} valueStyle={{ color: data.easytier.healthy ? "#368a32" : "#c74842" }} /></Card>
        <Card><Statistic title="私有网络" value={data.networkCount} suffix="/ 10" prefix={<NetworkIcon size={20} />} /></Card>
        <Card><Statistic title="设备成员" value={data.deviceCount} prefix={<Smartphone size={20} />} /></Card>
        <Card><Statistic title="当前在线" value={data.onlineCount} prefix={<RadioTower size={20} />} /></Card>
      </div>
      <Card title="服务信息">
        <Descriptions column={{ xs: 1, sm: 2, lg: 3 }}>
          <Descriptions.Item label="站点">{data.siteName}</Descriptions.Item>
          <Descriptions.Item label="模式"><ModeTag mode={data.mode} /></Descriptions.Item>
          <Descriptions.Item label="版本">{data.version}</Descriptions.Item>
          <Descriptions.Item label="Web 端口">{data.webPort}/TCP</Descriptions.Item>
          <Descriptions.Item label="EasyTier 端口">{data.easytierPort}/TCP+UDP</Descriptions.Item>
          <Descriptions.Item label="管理实例">{data.easytier.managerRunning}/{data.easytier.managerTotal}</Descriptions.Item>
        </Descriptions>
      </Card>
    </div>
  );
}

function NetworksPage() {
  const { info } = useAuth();
  const { message, modal } = AntApp.useApp();
  const [items, setItems] = useState<Network[]>([]);
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm();
  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.networks());
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setLoading(false);
    }
  }, [message]);
  useEffect(() => { void load(); }, [load]);
  const columns: ColumnsType<Network> = [
    { title: "网络名称", dataIndex: "name" },
    { title: "状态", render: (_, row) => <Tag color={row.status === "active" ? "green" : "default"}>{row.status === "active" ? "已启用" : "已停用"}</Tag> },
    { title: "设备", render: (_, row) => `${row.onlineCount} 在线 / ${row.deviceCount}` },
    { title: "创建时间", dataIndex: "createdAt", render: formatTime },
    {
      title: "操作",
      width: 250,
      render: (_, row) => (
        <Space wrap>
          <Button size="small" onClick={() => api.networkAction(row.id, row.status === "active" ? "disable" : "enable").then(load).catch((error) => message.error(error.message))}>
            {row.status === "active" ? "停用" : "启用"}
          </Button>
          <Button size="small" onClick={() => resetPassword(row)}>重置密码</Button>
          {row.status === "disabled" && <Popconfirm title="删除网络及其成员记录？" onConfirm={() => api.deleteNetwork(row.id).then(load).catch((error) => message.error(error.message))}><Button size="small" danger>删除</Button></Popconfirm>}
        </Space>
      ),
    },
  ];
  const resetPassword = (row: Network) => {
    let password = randomPassword();
    let adminPassword = "";
    modal.confirm({
      title: `重置“${row.name}”的网络密码`,
      content: <ResetPasswordFields
        initialPassword={password}
        onPasswordChange={(value) => { password = value; }}
        onAdminPasswordChange={(value) => { adminPassword = value; }}
      />,
      okText: "确认重置",
      okButtonProps: { danger: true },
      onOk: async () => {
        await api.resetNetworkPassword(row.id, { password, adminPassword });
        message.success("网络密码已重置，请妥善保存新密码");
      },
    });
  };
  return (
    <div className="page">
      <PageHeader
        title="私有网络"
        description="每个网络拥有独立密码、凭据和撤销边界。"
        action={(
          <Space>
            <Button icon={<RefreshCw size={16} />} loading={loading} onClick={() => void load()}>
              刷新
            </Button>
            <Button type="primary" icon={<Plus size={16} />} disabled={info.mode !== "private" || items.length >= 10} onClick={() => { form.resetFields(); form.setFieldValue("password", randomPassword()); setOpen(true); }}>
              新建网络
            </Button>
          </Space>
        )}
      />
      {info.mode === "public" && <Alert type="info" showIcon message="当前为公共节点模式" description="私有网络数据仍保留，但不会运行管理实例。" />}
      <Card className="table-card"><Table rowKey="id" columns={columns} dataSource={items} loading={loading} pagination={false} scroll={{ x: 760 }} locale={{ emptyText: <Empty description="暂无私有网络" /> }} /></Card>
      <Modal title="新建私有网络" open={open} onCancel={() => setOpen(false)} okText="创建" onOk={() => form.submit()}>
        <Form form={form} layout="vertical" requiredMark={false} onFinish={async (values) => { try { await api.createNetwork(values); message.success("网络创建成功"); setOpen(false); await load(); } catch (error) { message.error((error as Error).message); } }}>
          <Form.Item name="name" label="网络名称" rules={[{ required: true }, { max: 64 }]}><Input /></Form.Item>
          <PasswordField form={form} name="password" label="网络密码" />
          <Alert type="info" showIcon message="服务端只保存密码哈希，创建后无法找回，只能重置。" />
        </Form>
      </Modal>
    </div>
  );
}

function DevicesPage() {
  const { info } = useAuth();
  const { message, modal } = AntApp.useApp();
  const [items, setItems] = useState<Member[]>([]);
  const [networks, setNetworks] = useState<Network[]>([]);
  const [networkId, setNetworkId] = useState<string>();
  const load = useCallback(() => api.devices(networkId).then((value) => setItems(value.members)).catch((error) => message.error(error.message)), [message, networkId]);
  useEffect(() => { void api.networks().then(setNetworks); }, []);
  useEffect(() => { void load(); }, [load]);
  const columns: ColumnsType<Member> = [
    { title: "设备", render: (_, row) => <div><strong>{row.name ?? row.hostname ?? "-"}</strong><div className="subtle">{row.platform ?? row.version ?? ""}</div></div> },
    { title: "网络", dataIndex: "networkName", render: (value) => value || (info.mode === "public" ? "公共节点观测" : "-") },
    { title: "地址", render: (_, row) => row.virtualIp || row.ipv4 || "-" },
    { title: "状态", render: (_, row) => row.status === "revoked" ? <Tag color="red">已撤销</Tag> : <Tag color={row.online ? "green" : "default"}>{row.online ? "在线" : "离线"}</Tag> },
    { title: "流量", render: (_, row) => `${formatBytes(row.rxBytes)} ↓ / ${formatBytes(row.txBytes)} ↑` },
    {
      title: "操作",
      render: (_, row) => row.membershipId ? (
        <Space wrap>
          <Button size="small" onClick={() => editNote(row)}>备注</Button>
          {row.status !== "revoked" && <Popconfirm title="撤销后此设备不能重新加入该网络" onConfirm={() => api.revokeMembership(row.membershipId!).then(load).catch((error) => message.error(error.message))}><Button size="small" danger>撤销</Button></Popconfirm>}
          {row.status === "revoked" && !row.online && <Popconfirm title="删除该成员记录后，设备可重新加入" onConfirm={() => api.deleteMembership(row.membershipId!).then(load).catch((error) => message.error(error.message))}><Button size="small" danger>删除</Button></Popconfirm>}
        </Space>
      ) : null,
    },
  ];
  const editNote = (row: Member) => {
    let note = row.adminNote ?? "";
    modal.confirm({
      title: `编辑 ${row.name} 的管理员备注`,
      content: <Input.TextArea defaultValue={note} maxLength={100} showCount onChange={(event) => { note = event.target.value; }} />,
      onOk: () => api.updateMembership(row.membershipId!, { adminNote: note }).then(load),
    });
  };
  return (
    <div className="page">
      <PageHeader title="设备" description={info.mode === "private" ? "设备可属于多个网络，撤销操作只影响当前网络。" : "公共模式只显示共享节点观测到的聚合成员。"} action={<Button icon={<RefreshCw size={16} />} onClick={load}>刷新</Button>} />
      {info.mode === "private" && <Select allowClear placeholder="全部网络" value={networkId} options={networks.map((item) => ({ label: item.name, value: item.id }))} onChange={setNetworkId} className="filter-select" />}
      <Card className="table-card"><Table rowKey={(row) => row.membershipId ?? row.id ?? row.hostname!} columns={columns} dataSource={items} pagination={false} scroll={{ x: 900 }} /></Card>
    </div>
  );
}

function AuditPage() {
  const { message } = AntApp.useApp();
  const [data, setData] = useState<AuditResult>({ items: [], total: 0, page: 1, pageSize: 30 });
  const load = useCallback((page = data.page) => api.audit(page, data.pageSize).then(setData).catch((error) => message.error(error.message)), [data.page, data.pageSize, message]);
  useEffect(() => { void load(1); }, []);
  return (
    <div className="page">
      <PageHeader title="审计日志" description="保留 90 天，最多 10,000 条。" action={<Popconfirm title="清空全部审计日志？" onConfirm={() => api.clearAudit().then(() => load(1))}><Button danger icon={<X size={16} />}>清空</Button></Popconfirm>} />
      <Card className="table-card">
        <Table rowKey="id" dataSource={data.items} pagination={false} scroll={{ x: 800 }} columns={[
          { title: "时间", dataIndex: "createdAt", render: formatTime, width: 170 },
          { title: "来源", dataIndex: "actorType", width: 100 },
          { title: "操作", dataIndex: "action" },
          { title: "对象", render: (_, row) => `${row.targetType || "-"} ${row.targetId || ""}` },
          { title: "结果", dataIndex: "result", render: (value) => <Tag color={value === "success" ? "green" : "red"}>{value === "success" ? "成功" : "失败"}</Tag> },
        ]} />
        <Pagination current={data.page} pageSize={data.pageSize} total={data.total} showSizeChanger={false} onChange={(page) => load(page)} className="pager" />
      </Card>
    </div>
  );
}

function SettingsPage() {
  const { logout, refreshInfo } = useAuth();
  const { message, modal } = AntApp.useApp();
  const [settings, setSettings] = useState<Settings>();
  const [form] = Form.useForm();
  useEffect(() => { api.settings().then((value) => { setSettings(value); form.setFieldsValue(value); }).catch((error) => message.error(error.message)); }, [form, message]);
  if (!settings) return <Spin />;
  const changeMode = () => {
    let password = "";
    const nextMode = settings.mode === "private" ? "public" : "private";
    modal.confirm({
      title: `切换为${nextMode === "private" ? "私有节点" : "公共节点"}模式`,
      content: <Space direction="vertical" className="modal-fields"><Alert type="warning" showIcon message="切换会断开当前连接并重启 EasyTier。" /><Input.Password placeholder="管理员密码" onChange={(event) => { password = event.target.value; }} /></Space>,
      okText: "确认切换",
      onOk: async () => { await api.changeMode({ mode: nextMode, adminPassword: password }); message.success("模式已切换"); location.reload(); },
    });
  };
  return (
    <div className="page settings-page">
      <PageHeader title="设置" description="管理站点信息、管理员和节点模式。" />
      <Card title="站点与管理员">
        <Form form={form} layout="vertical" requiredMark={false} onFinish={async (values) => { try { const result = await api.updateSettings(values); await refreshInfo(); message.success("设置已保存"); if (result.reauthRequired) await logout(); } catch (error) { message.error((error as Error).message); } }}>
          <div className="form-grid">
            <Form.Item name="siteName" label="站点名称" rules={[{ required: true }, { max: 40 }]}><Input /></Form.Item>
            <Form.Item name="publicHost" label="对外 IP 或域名" rules={[{ required: true }]}><Input /></Form.Item>
            <Form.Item name="adminUsername" label="管理员用户名" rules={[{ required: true }]}><Input /></Form.Item>
            <Form.Item name="currentPassword" label="当前管理员密码"><Input.Password /></Form.Item>
            <Form.Item name="newPassword" label="新管理员密码" rules={[{ min: 8 }]}><Input.Password placeholder="不修改请留空" /></Form.Item>
          </div>
          <Button type="primary" htmlType="submit">保存设置</Button>
        </Form>
      </Card>
      <Card title="节点模式">
        <div className="setting-row">
          <div><strong>当前模式</strong><p>{settings.mode === "private" ? "私有网络由服务端认证并签发设备凭据。" : "共享节点转发用户自行设置的网络。"}</p></div>
          <Space><ModeTag mode={settings.mode} /><Button onClick={changeMode}>切换模式</Button></Space>
        </div>
      </Card>
      <Card title="运行参数">
        <Descriptions column={{ xs: 1, sm: 2 }}>
          <Descriptions.Item label="Web 端口">{settings.webPort}/TCP</Descriptions.Item>
          <Descriptions.Item label="EasyTier 端口">{settings.easytierPort}/TCP+UDP</Descriptions.Item>
          <Descriptions.Item label="服务端版本">{settings.version}</Descriptions.Item>
          <Descriptions.Item label="EasyTier 版本">{settings.easytierVersion}</Descriptions.Item>
        </Descriptions>
        <Alert type="info" showIcon message="端口由 Docker 环境变量配置，管理界面仅显示当前值。" />
      </Card>
      <Button danger icon={<LogOut size={16} />} onClick={logout}>退出登录</Button>
    </div>
  );
}

function ModeTag({ mode }: { mode: "public" | "private" }) {
  return <Tag icon={mode === "private" ? <ShieldCheck size={13} /> : <RadioTower size={13} />} color={mode === "private" ? "blue" : "green"}>{mode === "private" ? "私有节点" : "公共节点"}</Tag>;
}

const router = createHashRouter([
  {
    path: "/",
    element: <Shell />,
    children: [
      { index: true, element: <Navigate to="/overview" replace /> },
      { path: "overview", element: <OverviewPage /> },
      { path: "networks", element: <NetworksPage /> },
      { path: "devices", element: <DevicesPage /> },
      { path: "audit", element: <AuditPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
