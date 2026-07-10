import { useMemo } from "react";
import { createId } from "@/utils/id";

const key = "tong-net-client-id";

export function useDeviceIdentity() {
  return useMemo(() => {
    const existing = localStorage.getItem(key);

    if (existing) {
      return existing;
    }

    const next = createId();
    localStorage.setItem(key, next);
    return next;
  }, []);
}
