import { beforeEach, describe, expect, it, vi } from "vitest";
import { listMessages } from "@/api/message";
import type { Message } from "@/types/domain";
import { useUnreadStore } from "./unread";

vi.mock("@/api/message", () => ({ listMessages: vi.fn() }));
const request = vi.mocked(listMessages);
const item = (id: string, fromDeviceId = "phone", type: Message["type"] = "text"): Message => ({
  id, fromDeviceId, toDeviceId: "group", type, content: id, createdAt: id,
});
const respond = (data: Message[]) => request.mockResolvedValueOnce({ data } as Awaited<ReturnType<typeof listMessages>>);

beforeEach(() => {
  const storage = new Map<string, string>();
  vi.stubGlobal("localStorage", { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => storage.set(key, value) });
  request.mockReset();
  useUnreadStore.setState({ identityId: "", count: 0, cursor: undefined, initialized: false, visible: false });
  useUnreadStore.getState().configure("host");
});

describe("single group unread count", () => {
  it("counts messages from every other member, excluding own and system messages", async () => {
    respond([item("1")]);
    await useUnreadStore.getState().sync();
    expect(useUnreadStore.getState().count).toBe(0);
    respond([item("2", "phone"), item("3", "tablet"), item("4", "host"), item("5", "phone", "system")]);
    await useUnreadStore.getState().sync();
    expect(request).toHaveBeenLastCalledWith({ after: "1" });
    expect(useUnreadStore.getState().count).toBe(2);
    useUnreadStore.getState().setVisible(true);
    expect(useUnreadStore.getState().count).toBe(0);
    respond([item("6")]);
    await useUnreadStore.getState().sync();
    expect(useUnreadStore.getState().count).toBe(0);
  });
  it("counts the first incoming message after an empty initial history", async () => {
    respond([]);
    await useUnreadStore.getState().sync();
    respond([item("1")]);
    await useUnreadStore.getState().sync();
    expect(useUnreadStore.getState().count).toBe(1);
  });
  it("catches up across more than one page after disconnecting", async () => {
    respond([item("baseline")]);
    await useUnreadStore.getState().sync();
    respond(Array.from({ length: 50 }, (_, i) => item(String(i))));
    respond([item("last")]);
    await useUnreadStore.getState().sync();
    expect(useUnreadStore.getState().count).toBe(51);
    expect(useUnreadStore.getState().cursor).toBe("last");
  });
});
