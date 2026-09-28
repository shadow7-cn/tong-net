import { useCallback, useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Badge, Button, Layout, Menu, Tag, Tooltip, message } from "antd";
import { History, MessageCircle, MonitorCog, Network, RadioTower, Settings } from "lucide-react";
import { listDevices } from "@/api/device";
import { useLanSocket } from "@/hooks/useLanSocket";
import { setCurrentDeviceId } from "@/http";
import { useDeviceStore, useServiceStore, useUnreadStore } from "@/store";
import { useEasyTierStore } from "@/store/easytier";
import styles from "./index.module.less";

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [api, contextHolder] = message.useMessage();
  const running = useServiceStore((state) => state.running);
  const virtualConnected = useEasyTierStore((state) => state.connected);
  const refreshVirtualStatus = useEasyTierStore((state) => state.refresh);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try { await refreshVirtualStatus(); } catch { /* Retry on the next status check. */ }
      if (!disposed) timer = setTimeout(refresh, 3000);
    };
    void refresh();
    return () => { disposed = true; clearTimeout(timer); };
  }, [refreshVirtualStatus]);
  const loading = useServiceStore((state) => state.loading);
  const initialize = useServiceStore((state) => state.initialize);
  const startService = useServiceStore((state) => state.startService);
  const stopService = useServiceStore((state) => state.stopService);
  const setDevices = useDeviceStore((state) => state.setDevices);
  const totalUnread = useUnreadStore((state) => state.count);
  const configureUnread = useUnreadStore((state) => state.configure);
  const syncUnread = useUnreadStore((state) => state.sync);
  const setVisible = useUnreadStore((state) => state.setVisible);

  const refreshUnread = useCallback(async () => {
    if (!running) return;
    const { data } = await listDevices();
    setDevices(data);
    await syncUnread();
  }, [running, setDevices, syncUnread]);

  useEffect(() => { initialize().catch((error) => api.error(String(error))); }, [initialize]);
  useEffect(() => {
    if (!running) return;
    setCurrentDeviceId("host");
    configureUnread("host");
    void refreshUnread().catch(() => undefined);
  }, [configureUnread, refreshUnread, running]);
  useEffect(() => {
    if (location.pathname !== "/chat") setVisible(false);
  }, [location.pathname, setVisible]);

  useLanSocket(running, "host", () => { void refreshUnread().catch(() => undefined); });

  const menuItems = [
    {
      key: "lan-interconnect",
      icon: <RadioTower size={17} />,
      label: "局域网互通",
      children: [
        { key: "/desktop", icon: <MonitorCog size={17} />, label: "互通服务" },
        {
          key: "/chat",
          icon: <MessageCircle size={17} />,
          label: <span className={styles.menuLabel}>互通群聊<Badge count={totalUnread} overflowCount={99} size="small" /></span>,
        },
        { key: "/records", icon: <History size={17} />, label: "互通记录" },
      ],
    },
    { key: "/virtual-lan", icon: <Network size={17} />, label: <span className={styles.menuLabel}>虚拟局域网{virtualConnected && <Tooltip title="已连接"><span className={styles.connectedDot} role="status" aria-label="虚拟局域网已连接" /></Tooltip>}</span> },
    { key: "/settings", disabled: running, icon: <Settings size={17} />, label: <Tooltip title={running ? "请先停止互通服务，再进行设置" : undefined}><span style={{ display: "block" }}>设置</span></Tooltip> },
  ];

  const toggle = async () => {
    try { if (running) await stopService(); else await startService(); }
    catch (error) { api.error(String(error)); }
  };

  return (
    <Layout className={styles.shell}>
      {contextHolder}
      <Layout.Sider width={228} className={styles.sider}>
        <div className={styles.brand}>
          <img className={styles.brandMark} src="/brand/tong-net-logo.png" alt="同网互通 Logo" />
          <div>
            <div className={styles.brandName}>同网互通</div>
            <div className={styles.brandSub}>局域网临时传输站</div>
          </div>
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          defaultOpenKeys={["lan-interconnect"]}
          items={menuItems}
          onClick={(item) => navigate(item.key)}
          className={styles.menu}
        />
        <div className={styles.serviceBox}>
          <div className={styles.serviceLine}>
            <RadioTower size={16} />
            <span>互通服务</span>
            <Tag color={running ? "green" : "default"}>{running ? "运行中" : "未开启"}</Tag>
          </div>
          <Button type={running ? "default" : "primary"} block loading={loading} onClick={toggle}>
            {running ? "停止互通" : "开启互通"}
          </Button>
        </div>
      </Layout.Sider>
      <Layout.Content className={styles.content}>
        {running && location.pathname === "/settings" ? <div style={{ padding: 24 }}>请先停止互通服务，再进行设置。</div> : <Outlet />}
      </Layout.Content>
    </Layout>
  );
}
