import type { Message } from "@/types/domain";

export type ReadMarker = {
  id: string;
  createdAt: string;
};

type UnreadUpdate = {
  marker?: ReadMarker;
  unread: number;
};

export function advanceUnread(
  messages: Message[],
  peerId: string,
  marker: ReadMarker | undefined,
  unread: number,
  active: boolean,
): UnreadUpdate {
  const incoming = messages.filter((item) => item.fromDeviceId === peerId && item.type !== "system");
  const latest = incoming[incoming.length - 1];
  if (!latest) return { marker, unread: active ? 0 : unread };

  const nextMarker = { id: latest.id, createdAt: latest.createdAt };
  if (active) return { marker: nextMarker, unread: 0 };
  if (!marker) return { marker: nextMarker, unread };

  const markerIndex = incoming.findIndex((item) => item.id === marker.id);
  const unseen = markerIndex >= 0
    ? incoming.slice(markerIndex + 1)
    : incoming.filter((item) => item.createdAt > marker.createdAt);
  return { marker: nextMarker, unread: unread + unseen.length };
}
