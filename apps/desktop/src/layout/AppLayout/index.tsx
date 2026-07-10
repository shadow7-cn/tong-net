import { useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Button, Layout, Menu, Tag, message } from "antd";
import { History, MessageCircle, MonitorCog, RadioTower, Settings } from "lucide-react";
import { useServiceStore } from "@/store";
import styles from "./index.module.less";

const menuItems = [
  { key: "/desktop", icon: <MonitorCog size={17} />, label: "App 端" },
  { key: "/chat", icon: <MessageCircle size={17} />, label: "访问端会话" },
  { key: "/records", icon: <History size={17} />, label: "记录" },
  { key: "/settings", icon: <Settings size={17} />, label: "设置" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [api, contextHolder] = message.useMessage();
  const running = useServiceStore((state) => state.running);
  const loading = useServiceStore((state) => state.loading);
  const initialize = useServiceStore((state) => state.initialize);
  const startService = useServiceStore((state) => state.startService);
  const stopService = useServiceStore((state) => state.stopService);

  useEffect(() => { initialize().catch((error) => api.error(String(error))); }, [initialize]);

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
