import type { Message } from "@/types/domain";

export function mergeMessages(current: Message[], incoming: Message[], prepend = false): Message[] {
  const ordered = prepend ? [...incoming, ...current] : [...current, ...incoming];
  const messages = new Map(ordered.map((item) => [item.id, item]));
  return [...messages.values()].sort((a, b) => a.createdAt.localeCompare(b.createdAt));
}
