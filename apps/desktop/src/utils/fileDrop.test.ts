import { describe, expect, it } from "vitest";
import { isFileDrag, readDroppedFiles } from "./fileDrop";

describe("file drag and drop", () => {
  it("ignores text and link drags", () => {
    expect(isFileDrag({ types: ["text/plain", "text/uri-list"] })).toBe(false);
    expect(isFileDrag({ types: ["Files"] })).toBe(true);
  });

  it("keeps files, including empty ones, but skips directories", () => {
    const file = { name: "empty.txt", size: 0 } as File;
    const transfer = { items: [
      { kind: "file", webkitGetAsEntry: () => ({ isDirectory: false }), getAsFile: () => file },
      { kind: "file", webkitGetAsEntry: () => ({ isDirectory: true }), getAsFile: () => ({ name: "folder" }) },
      { kind: "string" },
    ], files: [] } as unknown as DataTransfer;
    expect(readDroppedFiles(transfer)).toEqual({ files: [file], hasDirectories: true });
  });

  it("supports browsers without entry APIs and file-list-only transfers", () => {
    const files = [{ name: "a.txt" }, { name: "b.pdf" }] as File[];
    expect(readDroppedFiles({ items: [], files } as unknown as DataTransfer).files).toEqual(files);
    expect(readDroppedFiles({ items: [{ kind: "file", getAsFile: () => files[0] }], files: [] } as unknown as DataTransfer).files).toEqual([files[0]]);
  });
});
