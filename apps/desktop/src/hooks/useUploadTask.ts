import { useState } from "react";

export function useUploadTask() {
  const [uploading, setUploading] = useState(false);

  const runUpload = async (task: () => Promise<unknown>) => {
    setUploading(true);
    try { return await task(); } finally { setUploading(false); }
  };

  return { uploading, runUpload };
}
