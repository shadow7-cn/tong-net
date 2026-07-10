import { describe, expect, it } from "vitest";
import { isNearScrollBottom } from "./scroll";
import { resolveLanSocketUrl } from "./socket";

describe("chat behavior", () => {
  it("uses the configured LAN service for the desktop WebSocket", () => {
    expect(resolveLanSocketUrl(
      "http://localhost:1420/#/chat",
      "http://127.0.0.1:7878/api",
      "token value",
      "host",
    )).toBe("ws://127.0.0.1:7878/ws?token=token%20value&deviceId=host");
  });

  it("recognizes a message list that is already near its bottom", () => {
    expect(isNearScrollBottom({ clientHeight: 500, scrollHeight: 1200, scrollTop: 685 })).toBe(true);
    expect(isNearScrollBottom({ clientHeight: 500, scrollHeight: 1200, scrollTop: 300 })).toBe(false);
  });
});
