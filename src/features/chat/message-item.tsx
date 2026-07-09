import { useMemo, useState } from "react";
import { PencilIcon, SmilePlusIcon, Trash2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import { fullTimestamp, timeOfDay } from "@/lib/format";
import type { Message } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useProfiles } from "@/stores/profiles";

const QUICK_EMOJIS = ["👍", "❤️", "😂", "😮", "😢", "🔥", "✅", "👀"];

export function MessageItem({
  message,
  showHeader,
}: {
  message: Message;
  showHeader: boolean;
}) {
  const myId = useAuth((s) => s.userId);
  const sender = useProfiles((s) => s.byId[message.sender_id]);
  const reactions = useChat((s) => s.reactions[message.id]);
  const [editing, setEditing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const isOwn = message.sender_id === myId;
  const isDeleted = message.deleted_at !== null;

  const grouped = useMemo(() => {
    const byEmoji = new Map<string, { count: number; mine: boolean }>();
    for (const r of reactions ?? []) {
      const entry = byEmoji.get(r.emoji) ?? { count: 0, mine: false };
      entry.count += 1;
      if (r.user_id === myId) entry.mine = true;
      byEmoji.set(r.emoji, entry);
    }
    return [...byEmoji.entries()];
  }, [reactions, myId]);

  return (
    <div
      className={cn(
        "group relative flex gap-2.5 px-4 py-0.5 hover:bg-muted/30",
        showHeader && "mt-1.5",
        message.pending && "opacity-60"
      )}
    >
      <div className="w-8 shrink-0 pt-0.5">
        {showHeader ? (
          <UserAvatar profile={sender} />
        ) : (
          <span className="block pt-1 text-right text-[10px] leading-4 text-muted-foreground/70 opacity-0 select-none group-hover:opacity-100">
            {timeOfDay(message.created_at)}
          </span>
        )}
      </div>

      <div className="min-w-0 flex-1">
        {showHeader && (
          <p className="flex items-baseline gap-2 leading-tight">
            <span className="text-sm font-medium">{sender?.display_name ?? "…"}</span>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="text-[10px] text-muted-foreground/70">
                  {timeOfDay(message.created_at)}
                </span>
              </TooltipTrigger>
              <TooltipContent>{fullTimestamp(message.created_at)}</TooltipContent>
            </Tooltip>
          </p>
        )}

        {isDeleted ? (
          <p className="text-sm text-muted-foreground/60 italic">Message deleted</p>
        ) : editing ? (
          <EditBox
            message={message}
            onDone={() => setEditing(false)}
          />
        ) : (
          <p className="text-sm leading-relaxed break-words whitespace-pre-wrap text-foreground/90">
            {message.content}
            {message.edited_at && (
              <span className="ml-1.5 text-[10px] text-muted-foreground/60">(edited)</span>
            )}
          </p>
        )}

        {!isDeleted && grouped.length > 0 && (
          <div className="mt-1 flex flex-wrap items-center gap-1">
            {grouped.map(([emoji, info]) => (
              <button
                key={emoji}
                className={cn(
                  "flex items-center gap-1 rounded-full border px-1.5 py-0.5 text-xs transition-colors",
                  info.mine
                    ? "border-primary/40 bg-primary/10"
                    : "border-border bg-muted/40 hover:border-muted-foreground/40"
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
            "absolute -top-3 right-4 items-center rounded-md border bg-popover p-0.5 shadow-sm",
            pickerOpen ? "flex" : "hidden group-hover:flex"
          )}
        >
          <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
            <PopoverTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Add reaction"
                className="text-muted-foreground">
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
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Edit message"
                className="text-muted-foreground"
                onClick={() => setEditing(true)}
              >
                <PencilIcon />
              </Button>
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
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            save();
          }
          if (e.key === "Escape") onDone();
        }}
      />
      <p className="mt-1 text-[10px] text-muted-foreground">
        escape to <button className="underline" onClick={onDone}>cancel</button> · enter to{" "}
        <button className="underline" onClick={save}>save</button>
      </p>
    </div>
  );
}
