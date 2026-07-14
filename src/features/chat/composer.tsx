import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  FilmIcon,
  ImageIcon,
  Loader2Icon,
  PaperclipIcon,
  ReplyIcon,
  SendIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { UserAvatar } from "@/components/user-avatar";
import { SettingsDialog } from "@/features/settings/settings-dialog";
import { Textarea } from "@/components/ui/textarea";
import { formattedBytes, prepareChatMedia, type PreparedMedia } from "@/lib/media";
import type { Message } from "@/lib/types";
import { useChat } from "@/stores/chat";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";
import { useProfiles } from "@/stores/profiles";

function replyExcerpt(message: Message) {
  if (message.deleted_at) return "Message deleted";
  if (message.content) return message.content;
  if (message.media_kind === "image") return "Image";
  if (message.media_kind === "video") return "Video";
  return "Message";
}

export function Composer({
  conversationId,
  placeholder,
}: {
  conversationId: string;
  placeholder: string;
}) {
  const [value, setValue] = useState("");
  const [media, setMedia] = useState<PreparedMedia | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [sending, setSending] = useState(false);
  const [progress, setProgress] = useState(0);
  const enterToSend = usePreferences((state) => state.enterToSend);
  const composerHintVisible = usePreferences((state) => state.composerHintVisible);
  const replyTo = useChat((state) => state.replyTo);
  const replySender = useProfiles((state) =>
    replyTo ? state.byId[replyTo.sender_id] : undefined
  );
  const ownProfile = useAuth((state) => state.profile);
  const ref = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const restoreFocusRef = useRef(false);

  useEffect(() => {
    if (!media) {
      setPreviewUrl(null);
      return;
    }
    const url = URL.createObjectURL(media.blob);
    setPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [media]);

  useLayoutEffect(() => {
    if (!restoreFocusRef.current) return;
    restoreFocusRef.current = false;
    ref.current?.focus({ preventScroll: true });
  }, [value]);

  const resize = () => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 160) + "px";
  };

  const send = async () => {
    const trimmed = value.trim();
    if ((!trimmed && !media) || preparing || sending) return;
    setSending(true);
    const sent = await useChat.getState().sendMessage(
      conversationId,
      trimmed,
      media ?? undefined,
      replyTo?.id ?? null
    );
    setSending(false);
    if (!sent) return;
    useChat.getState().setReplyTo(null);
    setValue("");
    setMedia(null);
    setProgress(0);
    requestAnimationFrame(resize);
  };

  const selectMedia = async (file: File | undefined) => {
    if (!file) return;
    setPreparing(true);
    setMedia(null);
    setProgress(0);
    try {
      const prepared = await prepareChatMedia(file, setProgress);
      setMedia(prepared);
      setProgress(1);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "This attachment could not be prepared.");
    } finally {
      setPreparing(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  return (
    <div className="shrink-0 px-0 pb-0">
      {replyTo && (
        <div className="mb-2 flex min-w-0 items-center gap-2 surface-panel app-surface border border-white/[0.12] bg-card/80 px-2.5 py-2">
          <ReplyIcon className="size-4 shrink-0 text-muted-foreground" />
          <div className="min-w-0 flex-1 border-l-2 border-white/[0.24] pl-2">
            <p className="text-[11px] font-medium text-foreground/85">
              Replying to {replySender?.display_name ?? "message"}
            </p>
            <p className="truncate text-xs text-muted-foreground">{replyExcerpt(replyTo)}</p>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Cancel reply"
            className="shrink-0 text-muted-foreground hover:bg-white/[0.07]"
            onClick={() => useChat.getState().setReplyTo(null)}
          >
            <XIcon />
          </Button>
        </div>
      )}

      {(preparing || media) && (
        <div className="mb-2 flex max-w-sm items-center gap-3 surface-panel app-surface border border-white/[0.15] bg-card p-2">
          <div className="flex size-14 shrink-0 items-center justify-center overflow-hidden rounded-md bg-muted">
            {preparing ? (
              <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
            ) : media?.kind === "image" && previewUrl ? (
              <img src={previewUrl} alt="" className="size-full object-cover" />
            ) : media?.kind === "video" && previewUrl ? (
              <video src={previewUrl} className="size-full object-cover" muted />
            ) : null}
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">
              {preparing ? "Compressing locally..." : media?.originalName}
            </p>
            <p className="text-xs text-muted-foreground">
              {preparing
                ? Math.round(progress * 100) + "%"
                : media
                  ? formattedBytes(media.blob.size) +
                    (media.kind === "video" ? " - 720p / 30 fps max" : " - WebP")
                  : ""}
            </p>
            {preparing && (
              <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full bg-foreground/60 transition-[width]"
                  style={{ width: Math.max(4, progress * 100) + "%" }}
                />
              </div>
            )}
          </div>
          {media && !sending && (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Remove attachment"
              onClick={() => setMedia(null)}
            >
              <XIcon />
            </Button>
          )}
        </div>
      )}

      <div className="surface-panel composer-surface floating-surface flex items-center gap-2 p-1.5 shadow-[0_10px_26px_rgb(0_0_0_/_0.20)] transition-colors focus-within:border-white/[0.28]">
        <input
          ref={fileRef}
          type="file"
          accept="image/*,video/*"
          className="hidden"
          onChange={(event) => void selectMedia(event.target.files?.[0])}
        />
        <Popover>
          <PopoverTrigger asChild>
            <button type="button" className="app-control flex h-9 shrink-0 items-center gap-1.5 px-2 text-left transition-colors hover:bg-white/[0.07]" aria-label="Your account and settings">
              <UserAvatar profile={ownProfile} size="sm" animated />

            </button>
          </PopoverTrigger>
          <PopoverContent side="top" align="start" className="surface-panel app-surface w-56 p-2.5">
            <div className="flex items-center gap-2 px-1 py-1.5">
              <UserAvatar profile={ownProfile} animated />
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{ownProfile?.display_name ?? "..."}</p>
                <p className="truncate text-xs text-muted-foreground">@{ownProfile?.username ?? ""}</p>
              </div>
            </div>
            <div className="mt-1 flex flex-col gap-0.5 border-t border-white/[0.11] pt-2">
              <SettingsDialog buttonLabel="Settings" />
              <Button variant="ghost" size="sm" className="w-full justify-start text-destructive hover:text-destructive" onClick={() => void useAuth.getState().signOut()}>Sign out</Button>
            </div>
          </PopoverContent>
        </Popover>        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Attach image or video"
          className="shrink-0 text-muted-foreground hover:bg-white/[0.07]"
          disabled={preparing || sending}
          onClick={() => fileRef.current?.click()}
        >
          {media?.kind === "image" ? (
            <ImageIcon />
          ) : media?.kind === "video" ? (
            <FilmIcon />
          ) : (
            <PaperclipIcon />
          )}
        </Button>
        <Textarea
          ref={ref}
          rows={1}
          value={value}
          placeholder={placeholder}
          maxLength={4000}
          className="max-h-40 min-h-9 flex-1 self-stretch resize-none border-0 bg-transparent py-2 leading-5 shadow-none focus-visible:ring-0 dark:bg-transparent"
          disabled={sending}
          onChange={(event) => {
            restoreFocusRef.current = document.activeElement === event.currentTarget;
            setValue(event.target.value);
            resize();
            useChat.getState().notifyTyping(conversationId);
          }}
          onKeyDown={(event) => {
            const shortcutSend = event.ctrlKey || event.metaKey;
            if (
              event.key === "Enter" &&
              (shortcutSend || (enterToSend && !event.shiftKey))
            ) {
              event.preventDefault();
              void send();
            }
          }}
        />
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Send message"
          className="shrink-0 text-muted-foreground hover:bg-white/[0.07]"
          disabled={preparing || sending || (!value.trim() && !media)}
          onClick={() => void send()}
        >
          {sending ? <Loader2Icon className="animate-spin" /> : <SendIcon />}
        </Button>
      </div>
      {composerHintVisible && (<div className="mt-1 flex min-h-3 items-center justify-between px-1 text-[10px] text-muted-foreground/60">
        <button className="hover:text-muted-foreground" onClick={() => usePreferences.getState().setPreference("composerHintVisible", false)}>{enterToSend ? "Enter to send - Shift+Enter for a new line" : "Ctrl+Enter to send"}</button>
        {value.length >= 3600 && (
          <span className={value.length >= 3900 ? "text-destructive" : undefined}>
            {4000 - value.length} left
          </span>
        )}
      </div>)}
      <p className="sr-only" aria-live="polite">
        {preparing ? "Compressing attachment locally." : sending ? "Sending message." : ""}
      </p>
    </div>
  );
}
