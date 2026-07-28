import { useCallback, useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Badge, Button, Layout, Menu, Tag, message } from "antd";
import { History, MessageCircle, MonitorCog, RadioTower, Settings } from "lucide-react";
import { listDevices } from "@/api/device";
import { useLanSocket } from "@/hooks/useLanSocket";
import { setCurrentDeviceId } from "@/http";
import { useDeviceStore, useServiceStore, useUnreadStore } from "@/store";
import styles from "./index.module.less";

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [api, contextHolder] = message.useMessage();
  const running = useServiceStore((state) => state.running);
  const loading = useServiceStore((state) => state.loading);
  const initialize = useServiceStore((state) => state.initialize);
  const startService = useServiceStore((state) => state.startService);
  const stopService = useServiceStore((state) => state.stopService);
  const setDevices = useDeviceStore((state) => state.setDevices);
  const unreadByPeer = useUnreadStore((state) => state.unreadByPeer);
  const configureUnread = useUnreadStore((state) => state.configure);
  const syncUnreadPeers = useUnreadStore((state) => state.syncPeers);
  const setActiveConversation = useUnreadStore((state) => state.setActiveConversation);
  const totalUnread = Object.values(unreadByPeer).reduce((sum, count) => sum + count, 0);

  const refreshUnread = useCallback(async () => {
    if (!running) return;
    const { data } = await listDevices();
    setDevices(data);
    await syncUnreadPeers(data.filter((device) => device.id !== "host"));
  }, [running, setDevices, syncUnreadPeers]);

  useEffect(() => { initialize().catch((error) => api.error(String(error))); }, [initialize]);
  useEffect(() => {
    if (!running) return;
    setCurrentDeviceId("host");
    configureUnread("host");
    void refreshUnread();
  }, [configureUnread, refreshUnread, running]);
  useEffect(() => {
    if (location.pathname !== "/chat") setActiveConversation("", false);
  }, [location.pathname, setActiveConversation]);

  useLanSocket(running, "host", () => { void refreshUnread(); });

  const menuItems = [
    { key: "/desktop", icon: <MonitorCog size={17} />, label: "App 端" },
    {
      key: "/chat",
      icon: <MessageCircle size={17} />,
      label: <span className={styles.menuLabel}>访问端会话<Badge count={totalUnread} overflowCount={99} size="small" /></span>,
    },
    { key: "/records", icon: <History size={17} />, label: "记录" },
    { key: "/settings", icon: <Settings size={17} />, label: "设置" },
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
          items={menuItems}
          onClick={(item) => navigate(item.key)}
          className={styles.menu}
        />
        <div className={styles.serviceBox}>
          <div className={styles.serviceLine}>
            <RadioTower size={16} />
            <span>局域网服务</span>
            <Tag color={running ? "green" : "default"}>{running ? "运行中" : "未开启"}</Tag>
          </div>
          <Button type={running ? "default" : "primary"} block loading={loading} onClick={toggle}>
            {running ? "停止互通" : "开启互通"}
          </Button>
        </div>
      </Layout.Sider>
      <Layout.Content className={styles.content}>
        <Outlet />
      </Layout.Content>
    </Layout>
  );
}
