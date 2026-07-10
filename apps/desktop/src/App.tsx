import { ConfigProvider } from "antd";
import zhCN from "antd/locale/zh_CN";
import { RouterProvider } from "react-router-dom";
import { router } from "@/router";
import "./global.less";
import "./theme.less";

export default function App() {
  return (
    <ConfigProvider locale={zhCN} theme={{ token: { borderRadius: 8, colorPrimary: "#0f766e" } }}>
      <RouterProvider router={router} />
    </ConfigProvider>
  );
}

