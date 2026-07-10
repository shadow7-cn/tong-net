import { request } from "@/http";
import type { Message } from "@/types/domain";

export function sendTextMessage(peerDeviceId: string, content: string) {
  return request.post<Message>(`/conversations/${peerDeviceId}/messages`, { content });
}
