import { Monitor, Smartphone } from "lucide-react";
import type { Device } from "@/types/domain";
import styles from "./index.module.less";

type DeviceAvatarProps = {
  device: Device;
  size?: "small" | "medium";
};

export default function DeviceAvatar({ device, size = "medium" }: DeviceAvatarProps) {
  const Icon = device.kind === "host" ? Monitor : Smartphone;

  return (
    <span className={`${styles.avatar} ${styles[size]} ${styles[device.status]}`}>
      <Icon size={size === "small" ? 16 : 20} strokeWidth={2} />
    </span>
  );
}

