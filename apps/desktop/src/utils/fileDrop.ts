export function isFileDrag(transfer: Pick<DataTransfer, "types">) {
  return Array.from(transfer.types).includes("Files");
}

export function readDroppedFiles(transfer: Pick<DataTransfer, "items" | "files">) {
  const items = Array.from(transfer.items ?? []).filter((item) => item.kind === "file");
  let hasDirectories = false;
  const files = items.length ? items.flatMap((item) => {
    if (item.webkitGetAsEntry?.()?.isDirectory) {
      hasDirectories = true;
      return [];
    }
    const file = item.getAsFile();
    return file ? [file] : [];
  }) : Array.from(transfer.files);
  return { files, hasDirectories };
}
