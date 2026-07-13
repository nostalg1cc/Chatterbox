import { useEffect, useRef, useState } from "react";
import {
  FilmIcon,
  ImageIcon,
  Loader2Icon,
  PaperclipIcon,
  SendIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  formattedBytes,
  prepareChatMedia,
  type PreparedMedia,
} from "@/lib/media";
import { useChat } from "@/stores/chat";
import { usePreferences } from "@/stores/preferences";

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
  const ref = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!media) {
      setPreviewUrl(null);
      return;
    }
    const url = URL.createObjectURL(media.blob);
    setPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [media]);

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
      media ?? undefined
    );
    setSending(false);
    if (!sent) return;
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
    <div className="shrink-0 px-4 pb-4">
      {(preparing || media) && (
        <div className="mb-2 flex max-w-sm items-center gap-3 rounded-lg border border-white/[0.15] bg-card p-2">
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

      <div className="flex items-end gap-1.5 rounded-lg border border-white/[0.15] bg-card px-2 py-2 transition-colors focus-within:border-white/[0.24]">
        <input
          ref={fileRef}
          type="file"
          accept="image/*,video/*"
          className="hidden"
          onChange={(event) => void selectMedia(event.target.files?.[0])}
        />
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Attach image or video"
          className="shrink-0 text-muted-foreground"
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
          className="max-h-40 min-h-6 flex-1 resize-none border-0 bg-transparent p-0 shadow-none focus-visible:ring-0 dark:bg-transparent"
          disabled={sending}
          onChange={(event) => {
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
          className="shrink-0 text-muted-foreground"
          disabled={preparing || sending || (!value.trim() && !media)}
          onClick={() => void send()}
        >
          {sending ? <Loader2Icon className="animate-spin" /> : <SendIcon />}
        </Button>
      </div>
      <p className="sr-only" aria-live="polite">
        {preparing ? "Compressing attachment locally." : sending ? "Sending message." : ""}
      </p>
    </div>
  );
}
