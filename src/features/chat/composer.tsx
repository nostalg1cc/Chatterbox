import { useRef, useState } from "react";
import { SendIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useChat } from "@/stores/chat";

export function Composer({
  conversationId,
  placeholder,
}: {
  conversationId: string;
  placeholder: string;
}) {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);

  const resize = () => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  };

  const send = () => {
    const trimmed = value.trim();
    if (!trimmed) return;
    setValue("");
    requestAnimationFrame(resize);
    void useChat.getState().sendMessage(conversationId, trimmed);
  };

  return (
    <div className="shrink-0 px-4 pb-4">
      <div className="flex items-end gap-1.5 rounded-lg border bg-card px-3 py-2 transition-colors focus-within:border-ring/60">
        <Textarea
          ref={ref}
          rows={1}
          value={value}
          placeholder={placeholder}
          maxLength={4000}
          className="max-h-40 min-h-6 flex-1 resize-none border-0 bg-transparent p-0 shadow-none focus-visible:ring-0 dark:bg-transparent"
          onChange={(e) => {
            setValue(e.target.value);
            resize();
            useChat.getState().notifyTyping(conversationId);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        />
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Send message"
          className="text-muted-foreground"
          disabled={!value.trim()}
          onClick={send}
        >
          <SendIcon />
        </Button>
      </div>
    </div>
  );
}
