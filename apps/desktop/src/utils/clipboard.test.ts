import { describe, expect, it, vi } from "vitest";
import { copyText } from "./clipboard";

describe("copyText", () => {
  it("uses Clipboard API in a secure browser", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    await copyText("download-url", { clipboard: { writeText }, document: {} as Document, secure: true });
    expect(writeText).toHaveBeenCalledWith("download-url");
  });

  it("falls back to execCommand on a LAN HTTP page", async () => {
    const textarea = {
      value: "",
      style: {},
      setAttribute: vi.fn(),
      select: vi.fn(),
      remove: vi.fn(),
    };
    const appendChild = vi.fn();
    const execCommand = vi.fn().mockReturnValue(true);
    const document = {
      body: { appendChild },
      createElement: vi.fn().mockReturnValue(textarea),
      execCommand,
    } as unknown as Document;

    await copyText("download-url", { document, secure: false });
    expect(textarea.value).toBe("download-url");
    expect(execCommand).toHaveBeenCalledWith("copy");
    expect(textarea.remove).toHaveBeenCalled();
  });
});
