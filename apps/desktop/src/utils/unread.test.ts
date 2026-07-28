import { describe, expect, it } from "vitest";
import type { Message } from "@/types/domain";
import { advanceUnread } from "./unread";

const message = (id: string, fromDeviceId: string, type: Message["type"] = "text"): Message => ({
  id,
  conversationId: "conversation",
  fromDeviceId,
  toDeviceId: "host",
  type,
  content: id,
  createdAt: `2026-07-21T00:00:0${id}.000Z`,
});

describe("unread conversation behavior", () => {
  it("uses existing history as the baseline without creating unread messages", () => {
    const result = advanceUnread([message("1", "phone")], "phone", undefined, 0, false);
    expect(result.unread).toBe(0);
    expect(result.marker?.id).toBe("1");
  });

  it("counts new peer messages but ignores own and system messages", () => {
    const result = advanceUnread(
      [message("1", "phone"), message("2", "host"), message("3", "phone", "system"), message("4", "phone")],
      "phone",
      { id: "1", createdAt: "2026-07-21T00:00:01.000Z" },
      2,
      false,
    );
    expect(result.unread).toBe(3);
    expect(result.marker?.id).toBe("4");
  });

  it("clears unread messages while the conversation is open", () => {
    const result = advanceUnread(
      [message("1", "phone"), message("2", "phone")],
      "phone",
      { id: "1", createdAt: "2026-07-21T00:00:01.000Z" },
      5,
      true,
    );
    expect(result.unread).toBe(0);
    expect(result.marker?.id).toBe("2");
  });
});
