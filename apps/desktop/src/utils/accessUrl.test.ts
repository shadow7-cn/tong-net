import { describe, expect, it } from "vitest";
import { buildAccessUrl } from "./accessUrl";

describe("buildAccessUrl", () => {
  it("生成带访问令牌的虚拟局域网地址", () => {
    expect(buildAccessUrl("10.126.126.2", 7878, "abc", true))
      .toBe("http://10.126.126.2:7878/?token=abc");
  });

  it("无令牌访问时不附加查询参数", () => {
    expect(buildAccessUrl("10.126.126.2", 7878, "abc", false))
      .toBe("http://10.126.126.2:7878/");
  });

  it("没有虚拟 IP 时不生成地址", () => {
    expect(buildAccessUrl("", 7878, "abc", true)).toBe("");
  });
});
