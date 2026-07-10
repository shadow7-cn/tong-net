import { describe, expect, it } from "vitest";
import { detectClientSource } from "./clientSource";

describe("detectClientSource", () => {
  it("prefers WeChat over the embedded Chrome marker", () => {
    expect(detectClientSource("Mozilla/5.0 Android Chrome/122 Mobile MicroMessenger/8.0")).toBe("微信内置浏览器");
  });

  it("recognizes common standalone browsers", () => {
    expect(detectClientSource("Mozilla/5.0 CriOS/122 Mobile Safari/604.1")).toBe("Chrome");
    expect(detectClientSource("Mozilla/5.0 Version/17.0 Mobile Safari/604.1")).toBe("Safari");
    expect(detectClientSource("Mozilla/5.0 EdgA/122 Chrome/122")).toBe("Edge");
  });
});
