import { describe, expect, it } from "vitest";
import { estimateRemainingSeconds, formatRemainingTime, formatTransferSpeed } from "./transfer";

describe("transfer formatting", () => {
  it("formats speed with a readable unit", () => {
    expect(formatTransferSpeed(1536)).toBe("1.5 KB/s");
    expect(formatTransferSpeed(5 * 1024 * 1024)).toBe("5.0 MB/s");
  });

  it("estimates and formats remaining time", () => {
    expect(estimateRemainingSeconds(10_000, 4_000, 1_000)).toBe(6);
    expect(formatRemainingTime(61)).toBe("约 2 分钟");
  });
});
