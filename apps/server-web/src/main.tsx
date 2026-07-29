import React from "react";
import ReactDOM from "react-dom/client";
import { App as AntApp, ConfigProvider } from "antd";
import zhCN from "antd/locale/zh_CN";
import App from "@/App";
import "@/global.less";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: "#0f857b",
          colorInfo: "#1677e8",
          colorSuccess: "#4b9b35",
          colorWarning: "#d48b19",
          colorError: "#cc4b45",
          borderRadius: 6,
          fontFamily:
            '"PingFang SC", "Microsoft YaHei", system-ui, -apple-system, "Segoe UI", sans-serif',
        },
      }}
    >
      <AntApp>
        <App />
      </AntApp>
    </ConfigProvider>
  </React.StrictMode>,
);
