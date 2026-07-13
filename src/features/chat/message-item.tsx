import { useEffect, useMemo, useState } from "react";
import {
  EyeOffIcon,
  ImageOffIcon,
  Loader2Icon,
  PencilIcon,
  SmilePlusIcon,
  Trash2Icon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import { fullTimestamp, timeOfDay } from "@/lib/format";
import { getCachedMedia, putCachedMedia } from "@/lib/media-cache";
import { nameColorClass } from "@/lib/name-colors";
import { supabase } from "@/lib/supabase";
import type { Message } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { usePreferences } from "@/stores/preferences";
import { useProfiles } from "@/stores/profiles";

const QUICK_EMOJIS = [
  "\u{1F44D}",
  "\u2764\uFE0F",
  "\u{1F602}",
  "\u{1F62E}",
  "\u{1F622}",
  "\u{1F525}",
  "\u2705",
  "\u{1F440}",
];

export function MessageItem({
  message,
  showHeader,
}: {
  message: Message;
  showHeader: boolean;
}) {
  const myId = useAuth((state) => state.userId);
  const sender = useProfiles((state) => state.byId[message.sender_id]);
  const reactions = useChat((state) => state.reactions[message.id]);
  const compact = usePreferences((state) => state.compactMessages);
  const showMediaPreviews = usePreferences((state) => state.showMediaPreviews);
  const [editing, setEditing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const isOwn = message.sender_id === myId;
  const isDeleted = message.deleted_at !== null;

  const grouped = useMemo(() => {
    const byEmoji = new Map<string, { count: number; mine: boolean }>();
    for (const reaction of reactions ?? []) {
      const entry = byEmoji.get(reaction.emoji) ?? { count: 0, mine: false };
      entry.count += 1;
      if (reaction.user_id === myId) entry.mine = true;
      byEmoji.set(reaction.emoji, entry);
    }
    return [...byEmoji.entries()];
  }, [reactions, myId]);

  return (
    <div
      className={cn(
        "group relative flex w-full min-w-0 max-w-full gap-2 px-4 transition-colors hover:bg-white/[0.045]",
        compact ? "py-0" : "py-0.5",
        showHeader && (compact ? "mt-1" : "mt-1.5"),
        message.pending && "opacity-60"
      )}
    >
      <div className="flex w-12 shrink-0 justify-center pt-0.5">
        {showHeader ? (
          <UserAvatar profile={sender} />
        ) : (
          <span className="block whitespace-nowrap pt-1 text-center text-[9px] leading-4 text-muted-foreground/75 opacity-0 tabular-nums select-none group-hover:opacity-100">
            {timeOfDay(message.created_at)}
          </span>
        )}
      </div>

      <div className="min-w-0 max-w-full flex-1 overflow-hidden">
        {showHeader && (
          <p className="flex min-w-0 items-baseline gap-2 leading-tight">
            <span className={cn("truncate text-sm font-medium", nameColorClass(sender?.name_color))}>
              {sender?.display_name ?? "..."}
            </span>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="shrink-0 whitespace-nowrap text-sm text-muted-foreground/75">
                  {timeOfDay(message.created_at)}
                </span>
              </TooltipTrigger>
              <TooltipContent>{fullTimestamp(message.created_at)}</TooltipContent>
            </Tooltip>
          </p>
        )}

        {isDeleted ? (
          <p className="text-sm text-muted-foreground/65 italic">Message deleted</p>
        ) : editing ? (
          <EditBox message={message} onDone={() => setEditing(false)} />
        ) : (
          <>
            {message.media_kind && (
              showMediaPreviews ? (
                <MediaAttachment message={message} />
              ) : (
                <div className="my-1.5 flex w-fit items-center gap-2 rounded-md border border-dashed border-white/[0.14] px-3 py-2 text-xs text-muted-foreground">
                  <EyeOffIcon className="size-4" />
                  Media preview hidden in Chat settings
                </div>
              )
            )}
            {message.content && (
              <p className="min-w-0 max-w-full whitespace-pre-wrap text-sm leading-relaxed text-foreground/95 [overflow-wrap:anywhere]">
                {message.content}
                {message.edited_at && (
                  <span className="ml-1.5 text-[10px] text-muted-foreground/65">
                    (edited)
                  </span>
                )}
              </p>
            )}
          </>
        )}

        {!isDeleted && grouped.length > 0 && (
          <div className="mt-1 flex flex-wrap items-center gap-1">
            {grouped.map(([emoji, info]) => (
              <button
                key={emoji}
                className={cn(
                  "flex items-center gap-1 rounded-full border px-1.5 py-0.5 text-xs transition-colors",
                  info.mine
                    ? "border-primary/45 bg-primary/12"
                    : "border-white/[0.14] bg-muted/50 hover:bg-muted/80 hover:text-foreground"
                )}
                onClick={() => void useChat.getState().toggleReaction(message, emoji)}
              >
                <span>{emoji}</span>
                <span className="text-muted-foreground tabular-nums">{info.count}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {!isDeleted && !editing && (
        <div
          className={cn(
            "absolute -top-3 right-4 items-center rounded-md border border-white/[0.15] bg-popover p-0.5 shadow-md",
            pickerOpen ? "flex" : "hidden group-hover:flex"
          )}
        >
          <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
            <PopoverTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Add reaction"
                className="text-muted-foreground"
              >
                <SmilePlusIcon />
              </Button>
            </PopoverTrigger>
            <PopoverContent className="w-auto p-1.5" align="end">
              <div className="flex gap-0.5">
                {QUICK_EMOJIS.map((emoji) => (
                  <Button
                    key={emoji}
                    variant="ghost"
                    size="icon-sm"
                    className="text-base"
                    onClick={() => {
                      void useChat.getState().toggleReaction(message, emoji);
                      setPickerOpen(false);
                    }}
                  >
                    {emoji}
                  </Button>
                ))}
              </div>
            </PopoverContent>
          </Popover>
          {isOwn && (
            <>
              {Boolean(message.content) && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Edit message"
                  className="text-muted-foreground"
                  onClick={() => setEditing(true)}
                >
                  <PencilIcon />
                </Button>
              )}
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Delete message"
                className="text-muted-foreground hover:text-destructive"
                onClick={() => void useChat.getState().deleteMessage(message.id)}
              >
                <Trash2Icon />
              </Button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function MediaAttachment({ message }: { message: Message }) {
  const userId = useAuth((state) => state.userId);
  const [url, setUrl] = useState<string | null>(null);
  const [local, setLocal] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!userId) return;
    let cancelled = false;
    let objectUrl: string | null = null;
    const abortController = new AbortController();

    setUrl(null);
    setLocal(false);
    setFailed(false);

    const load = async () => {
      try {
        const cached = await getCachedMedia(userId, message.id);
        if (cancelled) return;
        if (cached) {
          objectUrl = URL.createObjectURL(cached.blob);
          setUrl(objectUrl);
          setLocal(true);
          return;
        }

        if (!message.media_path) {
          setFailed(true);
          return;
        }

        const { data, error } = await supabase.storage
          .from("chat-media")
          .createSignedUrl(message.media_path, 60 * 60);
        if (cancelled) return;
        if (error || !data?.signedUrl) {
          setFailed(true);
          return;
        }

        setUrl(data.signedUrl);
        void fetch(data.signedUrl, { signal: abortController.signal })
          .then(async (response) => {
            if (!response.ok) return;
            const blob = await response.blob();
            await putCachedMedia({
              userId,
              messageId: message.id,
              blob,
              mimeType: message.media_mime_type ?? blob.type,
              createdAt: message.created_at,
            });
          })
          .catch((error: unknown) => {
            if (!(error instanceof DOMException && error.name === "AbortError")) {
              console.warn("Local media cache download failed", error);
            }
          });
      } catch (error) {
        if (!cancelled) setFailed(true);
        console.warn("Local media cache read failed", error);
      }
    };

    void load();

    return () => {
      cancelled = true;
      abortController.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [
    message.created_at,
    message.id,
    message.media_mime_type,
    message.media_path,
    userId,
  ]);

  if (failed) {
    return (
      <div className="my-1.5 flex w-fit items-center gap-2 rounded-md border border-dashed border-white/[0.14] px-3 py-2 text-xs text-muted-foreground">
        <ImageOffIcon className="size-4" />
        Attachment expired from server and local cache
      </div>
    );
  }

  if (!url) {
    return (
      <div className="my-1.5 flex h-24 w-40 items-center justify-center rounded-md border border-white/[0.13] bg-muted/35">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="relative my-1.5 w-fit max-w-full">
      {message.media_kind === "video" ? (
        <video
          src={url}
          controls
          playsInline
          preload="metadata"
          className="max-h-[420px] max-w-full rounded-lg border border-white/[0.14] bg-black"
        />
      ) : (
        <img
          src={url}
          alt="Chat attachment"
          loading="lazy"
          className="max-h-[420px] max-w-full rounded-lg border border-white/[0.14] object-contain"
        />
      )}
      {local && message.media_deleted_at && (
        <span className="absolute right-2 bottom-2 rounded bg-black/75 px-1.5 py-0.5 text-[10px] text-white/80">
          Saved locally
        </span>
      )}
    </div>
  );
}

function EditBox({ message, onDone }: { message: Message; onDone: () => void }) {
  const [value, setValue] = useState(message.content);

  const save = () => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== message.content) {
      void useChat.getState().editMessage(message.id, trimmed);
    }
    onDone();
  };

  return (
    <div className="mt-1">
      <Textarea
        autoFocus
        value={value}
        rows={1}
        className="min-h-9 resize-none"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            save();
          }
          if (event.key === "Escape") onDone();
        }}
      />
      <p className="mt-1 text-[10px] text-muted-foreground">
        escape to{" "}
        <button className="underline" onClick={onDone}>
          cancel
        </button>
        {" \u00B7 "}enter to{" "}
        <button className="underline" onClick={save}>
          save
        </button>
      </p>
    </div>
  );
}
