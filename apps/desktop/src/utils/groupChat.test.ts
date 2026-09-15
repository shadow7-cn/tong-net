import { describe, expect, it } from "vitest";
import type { Message } from "@/types/domain";
import { mergeMessages } from "./groupChat";

const message = (id: string): Message => ({
  id, fromDeviceId: "phone", toDeviceId: "group", type: "text", content: id,
  createdAt: `2026-09-16T00:00:0${id}.000Z`,
});

describe("group message merging", () => {
  it("deduplicates send responses and simultaneous socket refreshes", () => {
    expect(mergeMessages([message("1"), message("3")], [message("2"), message("3")]).map((item) => item.id)).toEqual(["1", "2", "3"]);
  });
  it("prepends history while retaining live messages", () => {
    expect(mergeMessages([message("3")], [message("1"), message("2")], true).map((item) => item.id)).toEqual(["1", "2", "3"]);
  });
});
