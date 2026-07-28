import { create } from "zustand";
import { listMessages } from "@/api/conversation";
import type { Device, Message } from "@/types/domain";
import { advanceUnread, type ReadMarker } from "@/utils/unread";

type PersistedUnread = {
  unreadByPeer: Record<string, number>;
  markers: Record<string, ReadMarker>;
};

type UnreadState = PersistedUnread & {
  identityId: string;
  activePeerId: string;
  conversationVisible: boolean;
  configure: (identityId: string) => void;
  ingestMessages: (peerId: string, messages: Message[]) => void;
  markRead: (peerId: string) => void;
  setActiveConversation: (peerId: string, visible: boolean) => void;
  syncPeers: (peers: Device[]) => Promise<void>;
};

const emptyPersisted = (): PersistedUnread => ({ unreadByPeer: {}, markers: {} });
const storageKey = (identityId: string) => `tong-net-unread:${identityId}`;

function loadPersisted(identityId: string): PersistedUnread {
  try {
    const value = localStorage.getItem(storageKey(identityId));
    return value ? { ...emptyPersisted(), ...JSON.parse(value) } : emptyPersisted();
  } catch {
    return emptyPersisted();
  }
}

function savePersisted(identityId: string, value: PersistedUnread) {
  if (!identityId) return;
  try { localStorage.setItem(storageKey(identityId), JSON.stringify(value)); } catch { /* Storage may be unavailable. */ }
}

export const useUnreadStore = create<UnreadState>((set, get) => ({
  ...emptyPersisted(),
  identityId: "",
  activePeerId: "",
  conversationVisible: false,
  configure: (identityId) => {
    if (!identityId || get().identityId === identityId) return;
    set({
      identityId,
      activePeerId: "",
      conversationVisible: false,
      ...loadPersisted(identityId),
    });
  },
  ingestMessages: (peerId, messages) => {
    const state = get();
    if (!state.identityId || !peerId) return;
    const active = state.conversationVisible && state.activePeerId === peerId;
    const update = advanceUnread(
      messages,
      peerId,
      state.markers[peerId],
      state.unreadByPeer[peerId] ?? 0,
      active,
    );
    const persisted = {
      unreadByPeer: { ...state.unreadByPeer, [peerId]: update.unread },
      markers: update.marker ? { ...state.markers, [peerId]: update.marker } : state.markers,
    };
    set(persisted);
    savePersisted(state.identityId, persisted);
  },
  markRead: (peerId) => {
    const state = get();
    if (!peerId || !state.unreadByPeer[peerId]) return;
    const persisted = {
      unreadByPeer: { ...state.unreadByPeer, [peerId]: 0 },
      markers: state.markers,
    };
    set(persisted);
    savePersisted(state.identityId, persisted);
  },
  setActiveConversation: (activePeerId, conversationVisible) => {
    set({ activePeerId, conversationVisible });
    if (conversationVisible && activePeerId) get().markRead(activePeerId);
  },
  syncPeers: async (peers) => {
    await Promise.all(peers.map(async (peer) => {
      const { data } = await listMessages(peer.id);
      get().ingestMessages(peer.id, data);
    }));
  },
}));
