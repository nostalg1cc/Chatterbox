import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDownIcon, Loader2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { dayLabel, sameDay } from "@/lib/format";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { MessageItem } from "./message-item";

const GROUP_WINDOW_MS = 5 * 60_000;

export function MessageList({
  conversationId,
  bottomInset = 0,
  mediaOnly = false,
}: {
  conversationId: string;
  bottomInset?: number;
  mediaOnly?: boolean;
}) {
  const allMessages = useChat((s) => s.messages[conversationId]);
  const messages = useMemo(
    () => mediaOnly ? allMessages?.filter((message) => Boolean(message.media_kind)) : allMessages,
    [allMessages, mediaOnly]
  );
  const hasMore = useChat((s) => s.hasMore[conversationId] ?? false);
  const loadingOlder = useChat((s) => s.loadingOlder);
  const myId = useAuth((s) => s.userId);

  const rootRef = useRef<HTMLDivElement | null>(null);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const atBottomRef = useRef(true);
  const lastCountRef = useRef(0);
  const [newBelow, setNewBelow] = useState(false);

  useEffect(() => {
    const vp = rootRef.current?.querySelector<HTMLDivElement>(
      '[data-slot="scroll-area-viewport"]'
    );
    viewportRef.current = vp ?? null;
    if (!vp) return;
    const onScroll = () => {
      const nearBottom = vp.scrollHeight - vp.scrollTop - vp.clientHeight < 60;
      atBottomRef.current = nearBottom;
      if (nearBottom) setNewBelow(false);
    };
    vp.addEventListener("scroll", onScroll);
    return () => vp.removeEventListener("scroll", onScroll);
  }, []);

  // Autoscroll: on first load, when already at the bottom, or on own sends.
  useEffect(() => {
    const vp = viewportRef.current;
    if (!vp || !messages) return;
    const prevCount = lastCountRef.current;
    lastCountRef.current = messages.length;
    if (messages.length <= prevCount) return;

    const last = messages[messages.length - 1];
    const shouldStick = prevCount === 0 || atBottomRef.current || last?.sender_id === myId;
    if (shouldStick) {
      requestAnimationFrame(() => {
        vp.scrollTop = vp.scrollHeight;
      });
    } else {
      setNewBelow(true);
    }
  }, [messages, myId]);
  useEffect(() => {
    const vp = viewportRef.current;
    if (!vp || !atBottomRef.current) return;
    requestAnimationFrame(() => {
      vp.scrollTop = vp.scrollHeight;
    });
  }, [bottomInset]);

  const scrollToBottom = () => {
    const vp = viewportRef.current;
    if (vp) vp.scrollTop = vp.scrollHeight;
    setNewBelow(false);
    useChat.getState().markRead(conversationId);
  };

  const loadOlder = async () => {
    const vp = viewportRef.current;
    if (!vp) return;
    const prevHeight = vp.scrollHeight;
    const prevTop = vp.scrollTop;
    await useChat.getState().loadOlder(conversationId);
    requestAnimationFrame(() => {
      vp.scrollTop = vp.scrollHeight - prevHeight + prevTop;
    });
  };

  if (!messages) {
    return (
      <div
        className="flex h-full min-h-0 flex-col justify-end gap-4 px-4"
        style={{ paddingBottom: bottomInset + 16 }}
      >
        {[56, 40, 72, 40].map((w, i) => (
          <div key={i} className="flex items-center gap-2.5">
            <Skeleton className="size-8 rounded-full" />
            <div className="flex-1 space-y-1.5">
              <Skeleton className="h-3 w-24" />
              <Skeleton className="h-3" style={{ width: `${w}%` }} />
            </div>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div ref={rootRef} className="relative h-full min-h-0">
      <ScrollArea className="h-full [&_[data-slot=scroll-area-viewport]]:overflow-x-hidden">
        <div
          className="flex w-full min-w-0 max-w-full flex-col overflow-x-hidden pt-3"
          style={{ paddingBottom: bottomInset + 8 }}
        >
          {hasMore && (
            <div className="flex justify-center pb-2">
              <Button
                variant="ghost"
                size="xs"
                className="text-muted-foreground"
                disabled={loadingOlder}
                onClick={() => void loadOlder()}
              >
                {loadingOlder && <Loader2Icon className="animate-spin" />}
                Load earlier messages
              </Button>
            </div>
          )}
          {messages.length === 0 && (
            <div className="px-4 py-12 text-center">
              <p className="text-sm font-medium text-muted-foreground">
                {mediaOnly ? "No shared media yet" : "This is the beginning of your conversation"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground/65">
                {mediaOnly
                  ? "Images and videos from this DM will appear here."
                  : "Send a message to get things started."}
              </p>
            </div>
          )}
          {messages.map((msg, i) => {
            const prev = messages[i - 1];
            const newDay = !prev || !sameDay(prev.created_at, msg.created_at);
            const grouped =
              !newDay &&
              prev !== undefined &&
              prev.sender_id === msg.sender_id &&
              new Date(msg.created_at).getTime() - new Date(prev.created_at).getTime() <
                GROUP_WINDOW_MS;
            return (
              <div key={msg.id}>
                {newDay && <DaySeparator iso={msg.created_at} />}
                <MessageItem message={msg} showHeader={!grouped} />
              </div>
            );
          })}
        </div>
      </ScrollArea>

      {newBelow && (
        <div
          className="absolute inset-x-0 flex justify-center"
          style={{ bottom: bottomInset + 8 }}
        >
          <Button size="sm" variant="secondary" className="shadow-md" onClick={scrollToBottom}>
            <ArrowDownIcon />
            New messages
          </Button>
        </div>
      )}
    </div>
  );
}

function DaySeparator({ iso }: { iso: string }) {
  return (
    <div className="flex items-center gap-3 px-4 py-3">
      <Separator className="flex-1" />
      <span className="shrink-0 text-[11px] font-medium text-muted-foreground">
        {dayLabel(iso)}
      </span>
      <Separator className="flex-1" />
    </div>
  );
}
