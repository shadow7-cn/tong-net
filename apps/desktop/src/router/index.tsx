import { Navigate, createHashRouter } from "react-router-dom";
import AppLayout from "@/layout/AppLayout";
import DesktopHome from "@/views/DesktopHome";
import NotFound from "@/views/NotFound";
import Records from "@/views/Records";
import Settings from "@/views/Settings";
import WebClient from "@/views/WebClient";
import { isTauri } from "@/api/service";
import { getDefaultRoute } from "@/utils/route";

function RootRedirect() {
  return <Navigate to={getDefaultRoute(isTauri())} replace />;
}

export const router = createHashRouter([
  {
    path: "/",
    element: <RootRedirect />,
  },
  {
    path: "/web",
    element: <WebClient />,
  },
  {
    element: <AppLayout />,
    children: [
      {
        path: "/desktop",
        element: <DesktopHome />,
      },
      {
        path: "/records",
        element: <Records />,
      },
      {
        path: "/chat",
        element: <WebClient hostMode />,
      },
      {
        path: "/settings",
        element: <Settings />,
      },
    ],
  },
  {
    path: "*",
    element: <NotFound />,
  },
]);
