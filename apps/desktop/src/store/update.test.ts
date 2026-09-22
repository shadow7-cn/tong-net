import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ check: vi.fn(), isTauri: vi.fn(), version: vi.fn() }));
vi.mock("@/api/update", () => ({ checkForUpdates: mocks.check }));
vi.mock("@/api/service", () => ({ isTauri: mocks.isTauri }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mocks.version }));

const result = { currentVersion: "0.2.2", latestVersion: "0.2.3", available: true, notes: "更新", releaseUrl: "https://github.com/shadow7-cn/tong-net/releases/tag/v0.2.3" };

beforeEach(() => {
  vi.resetModules();
  vi.resetAllMocks();
  mocks.isTauri.mockReturnValue(true);
  mocks.version.mockResolvedValue("0.2.2");
  mocks.check.mockResolvedValue(result);
});

it("checks once on startup, deduplicates requests and allows manual recheck", async () => {
  const { useUpdateStore } = await import("./update");
  const store = useUpdateStore.getState();
  store.start();
  store.start();
  await store.check();
  expect(mocks.check).toHaveBeenCalledTimes(1);
  expect(useUpdateStore.getState().availableUpdate).toEqual(result);
  store.dismiss();
  store.start();
  expect(useUpdateStore.getState().availableUpdate).toBeNull();
  await store.check();
  expect(mocks.check).toHaveBeenCalledTimes(2);
  expect(useUpdateStore.getState().checking).toBe(false);
});

it("never checks automatically in a web browser", async () => {
  mocks.isTauri.mockReturnValue(false);
  const { useUpdateStore } = await import("./update");
  useUpdateStore.getState().start();
  expect(mocks.check).not.toHaveBeenCalled();
  expect(mocks.version).not.toHaveBeenCalled();
});

it("silently handles automatic failure and allows manual retry", async () => {
  mocks.check.mockRejectedValueOnce("网络失败");
  const { useUpdateStore } = await import("./update");
  useUpdateStore.getState().start();
  await vi.waitFor(() => expect(useUpdateStore.getState().checking).toBe(false));
  expect(useUpdateStore.getState().availableUpdate).toBeNull();
  expect(useUpdateStore.getState().currentVersion).toBe("0.2.2");
  await useUpdateStore.getState().check();
  expect(useUpdateStore.getState().availableUpdate).toEqual(result);
});

it("does not show a prompt when no update is available", async () => {
  mocks.check.mockResolvedValue({ ...result, available: false });
  const { useUpdateStore } = await import("./update");
  await useUpdateStore.getState().check();
  expect(useUpdateStore.getState().availableUpdate).toBeNull();
});
