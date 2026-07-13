export type NameColor =
  | "default"
  | "slate"
  | "red"
  | "orange"
  | "amber"
  | "green"
  | "cyan"
  | "blue"
  | "violet"
  | "pink";

export interface Profile {
  id: string;
  username: string;
  display_name: string;
  created_at: string;
  avatar_path: string | null;
  avatar_updated_at: string | null;
  name_color: NameColor;
}

export type FriendshipStatus = "pending" | "accepted" | "blocked";

export interface Friendship {
  id: string;
  requester_id: string;
  addressee_id: string;
  status: FriendshipStatus;
  created_at: string;
  updated_at: string;
}

export interface Conversation {
  id: string;
  user1_id: string;
  user2_id: string;
  created_at: string;
  last_message_at: string;
}

export type MediaKind = "image" | "video";

export interface Message {
  id: string;
  conversation_id: string;
  sender_id: string;
  content: string;
  created_at: string;
  edited_at: string | null;
  deleted_at: string | null;
  media_kind: MediaKind | null;
  media_path: string | null;
  media_mime_type: "image/webp" | "video/webm" | null;
  media_size_bytes: number | null;
  media_width: number | null;
  media_height: number | null;
  media_duration_seconds: number | null;
  media_expires_at: string | null;
  media_deleted_at: string | null;
  /** Client-only: true while an optimistic send is in flight. */
  pending?: boolean;
}

export interface Reaction {
  id: string;
  message_id: string;
  user_id: string;
  emoji: string;
  created_at: string;
}

export interface ConversationOverview {
  conversation_id: string;
  last_message_id: string | null;
  last_message_content: string | null;
  last_message_sender_id: string | null;
  last_message_deleted: boolean | null;
  last_message_at: string | null;
  unread_count: number;
}

export interface VoiceRoom {
  conversation_id: string;
  generation: string;
  started_at: string;
  started_by: string;
  updated_at: string;
}

export interface VoiceParticipant {
  conversation_id: string;
  user_id: string;
  session_id: string;
  joined_at: string;
  last_seen_at: string;
  sharing_screen: boolean;
}

export type VoiceConnectionStatus =
  | "idle"
  | "joining"
  | "solo"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "failed";

export interface VoicePresence {
  userId: string;
  sessionId: string;
  muted: boolean;
  deafened: boolean;
  sharingScreen: boolean;
  joinedAt: string;
}

export type VoiceSignal =
  | {
      version: 1;
      generation: string;
      fromSessionId: string;
      toSessionId?: string;
      type: "ready";
    }
  | {
      version: 1;
      generation: string;
      fromSessionId: string;
      toSessionId?: string;
      type: "description";
      description: RTCSessionDescriptionInit;
    }
  | {
      version: 1;
      generation: string;
      fromSessionId: string;
      toSessionId?: string;
      type: "ice-candidate";
      candidate: RTCIceCandidateInit;
    };