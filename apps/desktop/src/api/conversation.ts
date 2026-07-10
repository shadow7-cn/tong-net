import { request } from "@/http";
import type { Message } from "@/types/domain";

export function listMessages(peerDeviceId: string) {
  return request.get<Message[]>(`/conversations/${peerDeviceId}/messages`);
}
