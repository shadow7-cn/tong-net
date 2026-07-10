import { useEffect, useRef, useState } from "react";
import { getAccessToken, request } from "@/http";
import { resolveLanSocketUrl } from "@/utils/socket";

export function useLanSocket(enabled: boolean, deviceId: string, onEvent: () => void) {
  const [connected, setConnected] = useState(false);
  const callback = useRef(onEvent);
  callback.current = onEvent;

  useEffect(() => {
    if (!enabled || !deviceId) {
      setConnected(false);
      return;
    }
    let disposed = false;
    let socket: WebSocket | undefined;
    let retry: number | undefined;
    const connect = () => {
      socket = new WebSocket(resolveLanSocketUrl(
        window.location.href,
        request.defaults.baseURL,
        getAccessToken(),
        deviceId,
      ));
      socket.onopen = () => setConnected(true);
      socket.onmessage = () => callback.current();
      socket.onerror = () => console.warn("同网互通实时连接发生错误");
      socket.onclose = (event) => {
        if (event.code !== 1000) console.warn("同网互通实时连接已关闭", event.code, event.reason);
        setConnected(false);
        if (!disposed) retry = window.setTimeout(connect, 1500);
      };
    };
    connect();
    return () => {
      disposed = true;
      if (retry) window.clearTimeout(retry);
      socket?.close();
      setConnected(false);
    };
  }, [deviceId, enabled]);

  return { connected };
}
