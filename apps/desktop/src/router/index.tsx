import { Navigate, createHashRouter } from "react-router-dom";
import AppLayout from "@/layout/AppLayout";
import DesktopHome from "@/views/DesktopHome";
import NotFound from "@/views/NotFound";
import Records from "@/views/Records";
import Settings from "@/views/Settings";
import WebClient from "@/views/WebClient";

export const router = createHashRouter([
  {
    path: "/web",
    element: <WebClient />,
  },
  {
    path: "/",
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <Navigate to="/desktop" replace />,
      },
      {
        path: "desktop",
        element: <DesktopHome />,
      },
      {
        path: "records",
        element: <Records />,
      },
      {
        path: "chat",
        element: <WebClient hostMode />,
      },
      {
        path: "settings",
        element: <Settings />,
      },
    ],
  },
  {
    path: "*",
    element: <NotFound />,
  },
]);
