type ScrollMetrics = Pick<HTMLElement, "clientHeight" | "scrollHeight" | "scrollTop">;

export function isNearScrollBottom(metrics: ScrollMetrics, threshold = 32) {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight <= threshold;
}
