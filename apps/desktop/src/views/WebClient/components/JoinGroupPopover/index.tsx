import { Button, Popover, QRCode, Tooltip, message } from "antd";
import { Copy, QrCode } from "lucide-react";
import { copyText } from "@/utils/clipboard";
import styles from "./index.module.less";

export default function JoinGroupPopover({ url, label = "局域网地址" }: { url: string; label?: string }) {
  const [api, contextHolder] = message.useMessage();

  const copyAddress = async () => {
    try {
      await copyText(url);
      api.success(`${label}已复制`);
    } catch {
      api.error("复制失败，请手动选择地址复制");
    }
  };

  return <>
    {contextHolder}
    <Popover
      title="扫码加入群聊"
      placement="bottomRight"
      trigger={["hover", "click", "focus"]}
      mouseLeaveDelay={0.2}
      content={<div className={styles.content}>
        <QRCode value={url} size={208} bordered={false} errorLevel="M" />
        <div className={styles.label}>{label}</div>
        <div className={styles.addressRow}>
          <div className={styles.address} tabIndex={0}>{url}</div>
          <Tooltip title={`复制${label}`}>
            <Button aria-label={`复制${label}`} icon={<Copy size={16} />} onClick={copyAddress} />
          </Tooltip>
        </div>
      </div>}
    >
      <Button size="small" type="text" aria-label={`${label}二维码`} icon={<QrCode size={17} />} />
    </Popover>
  </>;
}
