import { describe, expect, it, vi } from "vitest";
import type { EasyTierConfig, EasyTierMember } from "@/api/easytier";
import { continueInsecureConnection, getEasyTierMemberRole } from "./easytier";

const config: EasyTierConfig = {
  serverUrl: "http://vpn.example.com",
  networkName: "test",
  networkPassword: "secret",
  deviceName: "desktop",
  allowInsecureHttp: false,
};

const member = (hostname: string, local = false): EasyTierMember => ({
  id: hostname,
  hostname,
  ipv4: "",
  cost: "",
  latency: "",
  lossRate: "",
  rxBytes: "",
  txBytes: "",
  protocol: "",
  natType: "",
  version: "",
  local,
});

describe("continueInsecureConnection", () => {
  it("starts connecting without returning the pending connection promise", () => {
    const rememberConfirmation = vi.fn();
    const connect = vi.fn(() => new Promise<void>(() => undefined));

    const result = continueInsecureConnection(config, rememberConfirmation, connect);

    expect(result).toBeUndefined();
    expect(rememberConfirmation).toHaveBeenCalledWith({
      ...config,
      allowInsecureHttp: true,
    });
    expect(connect).toHaveBeenCalledWith({
      ...config,
      allowInsecureHttp: true,
    });
  });
});

describe("getEasyTierMemberRole", () => {
  it("recognizes local, shared and network service nodes", () => {
    expect(getEasyTierMemberRole(member("desktop", true))).toBe("local");
    expect(getEasyTierMemberRole(member("PublicServer_同网互通服务-共享节点"))).toBe("shared");
    expect(getEasyTierMemberRole(member("同网互通服务-test"))).toBe("service");
    expect(getEasyTierMemberRole(member("phone"))).toBe("device");
  });
});
