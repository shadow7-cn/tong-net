import { request } from "@/http";
import type { Message } from "@/types/domain";

export function listMessages(params?: { before?: string; after?: string }) {
  return request.get<Message[]>("/group/messages", { params });
}

export function sendTextMessage(content: string) {
  return request.post<Message>("/group/messages", { content });
}
