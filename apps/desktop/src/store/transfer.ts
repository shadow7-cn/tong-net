import { create } from "zustand";
import { request } from "@/http";
import type { TransferTask } from "@/types/domain";

type TransferState = {
  transfers: TransferTask[];
  loadTransfers: () => Promise<void>;
};

export const useTransferStore = create<TransferState>((set) => ({
  transfers: [],
  loadTransfers: async () => set({ transfers: (await request.get<TransferTask[]>("/transfers")).data }),
}));
