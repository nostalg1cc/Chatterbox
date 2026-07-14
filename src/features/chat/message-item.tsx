import { useEffect, useMemo, useState } from "react";
import {
  EyeOffIcon,
  ImageOffIcon,
  Loader2Icon,
  PencilIcon,
  ReplyIcon,
  SmilePlusIcon,
  Trash2Icon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import { DecoratedText } from "@/components/decorated-text";
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
  animateDecoration = false,
  decorationActive = false,
  onDecorationHoverChange,
}: {
  message: Message;
  showHeader: boolean;
  animateDecoration?: boolean;
  decorationActive?: boolean;
  onDecorationHoverChange?: (hovered: boolean) => void;
}) {
  const myId = useAuth((state) => state.userId);
  const sender = useProfiles((state) => state.byId[message.sender_id]);
  const reactions = useChat((state) => state.reactions[message.id]);
  const replyTarget = useChat((state) =>
    message.reply_to_message_id ? state.replyTargets[message.reply_to_message_id] : undefined
  );
  const replySender = useProfiles((state) =>
    replyTarget ? state.byId[replyTarget.sender_id] : undefined
  );
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
      id={`message-${message.id}`}
      onPointerEnter={() => onDecorationHoverChange?.(true)}
      onPointerLeave={() => onDecorationHoverChange?.(false)}
      className={cn(
        "group relative mx-auto flex w-full max-w-[1120px] min-w-0 gap-2 px-5 transition-colors hover:bg-white/[0.025]",
        isOwn && "flex-row-reverse",
        compact ? "py-0.5" : "py-1",
        showHeader && (compact ? "mt-2" : "mt-3"),
        message.pending && "opacity-60"
      )}
    >
      <div className="flex w-12 shrink-0 justify-center pt-0.5">
        {showHeader ? (
          <UserAvatar profile={sender} animated={animateDecoration} decorationActive={decorationActive} />
        ) : (
          <span className="block whitespace-nowrap pt-1 text-center text-[9px] leading-4 text-muted-foreground/75 opacity-0 tabular-nums select-none group-hover:opacity-100">
            {timeOfDay(message.created_at)}
          </span>
        )}
      </div>

      <div className={cn("min-w-0 max-w-full flex-1 overflow-hidden", isOwn && "text-right")}>
        {showHeader && (
          <p className={cn("flex min-w-0 items-baseline gap-2 leading-tight", isOwn && "justify-end")}>
            <span className={cn("truncate text-sm font-medium", nameColorClass(sender?.name_color))}>
              <DecoratedText effect={sender?.name_decoration as never} font={sender?.name_font} weight={sender?.name_weight} active>{sender?.display_name ?? "..."}</DecoratedText>
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

        {message.reply_to_message_id && (
          <ReplyPreview target={replyTarget} senderName={replySender?.display_name} />
        )}

        {isDeleted ? (
          <p className="text-sm text-muted-foreground/65 italic">Message deleted</p>
        ) : editing ? (
          <EditBox message={message} onDone={() => setEditing(false)} />
        ) : (
          <>
            {message.media_kind && (
              showMediaPreviews ? (
                <MediaAttachment message={message} alignEnd={isOwn} />
              ) : (
                <div className="my-1.5 flex w-fit items-center gap-2 rounded-md border border-dashed border-white/[0.14] px-3 py-2 text-xs text-muted-foreground">
                  <EyeOffIcon className="size-4" />
                  Media preview hidden in Chat settings
                </div>
              )
            )}
            {message.content && (
              <p className={cn("min-w-0 max-w-full whitespace-pre-wrap text-sm leading-relaxed text-foreground/95 [overflow-wrap:anywhere]", isOwn && "ml-auto")}>
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
          <div className={cn("mt-1 flex flex-wrap items-center gap-1", isOwn && "justify-end")}>
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
            "composer-action-tray absolute -top-2 items-center",
            isOwn ? "left-4" : "right-4",
            pickerOpen ? "action-tray-open" : ""
          )}
        >
          <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
            <PopoverTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Add reaction"
                className="action-tray-button text-muted-foreground"
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
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Reply to message"
            className="action-tray-button text-muted-foreground"
            onClick={() => useChat.getState().setReplyTo(message)}
          >
            <ReplyIcon />
          </Button>
          {isOwn && (
            <>
              {Boolean(message.content) && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Edit message"
                  className="action-tray-button text-muted-foreground"
                  onClick={() => setEditing(true)}
                >
                  <PencilIcon />
                </Button>
              )}
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Delete message"
                className="action-tray-button text-muted-foreground hover:text-destructive"
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

function ReplyPreview({
  target,
  senderName,
}: {
  target: Message | undefined;
  senderName: string | undefined;
}) {
  const excerpt = target
    ? target.deleted_at
      ? "Message deleted"
      : target.content || (target.media_kind === "image" ? "Image" : target.media_kind === "video" ? "Video" : "Message")
    : "Original message unavailable";

  return (
    <button
      type="button"
      className="mt-1 mb-1 flex min-w-0 max-w-full items-center gap-1.5 rounded-md border border-white/[0.1] bg-muted/[0.18] px-2 py-1 text-left transition-colors hover:bg-muted/[0.35]"
      onClick={() => target && document.getElementById(`message-${target.id}`)?.scrollIntoView({ behavior: "smooth", block: "center" })}
      disabled={!target}
      aria-label={target ? "Jump to replied message" : "Original message unavailable"}
    >
      <ReplyIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0">
        <span className="mr-1 text-[11px] font-medium text-foreground/80">{senderName ?? "Message"}</span>
        <span className="truncate text-xs text-muted-foreground">{excerpt}</span>
      </span>
    </button>
  );
}
function MediaAttachment({ message, alignEnd = false }: { message: Message; alignEnd?: boolean }) {
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
    <div className={cn("my-1.5 w-fit max-w-full", alignEnd && "ml-auto")}>
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
        <span className="mt-1 flex w-fit items-center rounded-full border border-white/[0.12] bg-muted/70 px-2 py-0.5 text-[10px] text-muted-foreground">
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