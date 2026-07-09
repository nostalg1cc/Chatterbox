import { useState } from "react";
import { toast } from "sonner";
import { Loader2Icon, UserPlusIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useAuth } from "@/stores/auth";
import { useFriends } from "@/stores/friends";

export function AddFriendDialog() {
  const [open, setOpen] = useState(false);
  const [username, setUsername] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const { userId, profile } = useAuth.getState();
    if (!userId || !profile) return;
    setLoading(true);
    setError(null);
    try {
      await useFriends.getState().sendRequest(username, userId, profile.username);
      toast.success(`Friend request sent to @${username.trim().toLowerCase()}`);
      setUsername("");
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) {
          setUsername("");
          setError(null);
        }
      }}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button variant="ghost" size="icon" aria-label="Add friend">
              <UserPlusIcon />
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent>Add friend</TooltipContent>
      </Tooltip>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Add a friend</DialogTitle>
          <DialogDescription>
            Send a request using their exact username.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="add-friend-username">Username</Label>
            <Input
              id="add-friend-username"
              autoFocus
              autoComplete="off"
              spellCheck={false}
              placeholder="username"
              value={username}
              onChange={(e) => {
                setUsername(e.target.value.toLowerCase());
                setError(null);
              }}
            />
            {error && <p className="text-xs text-destructive">{error}</p>}
          </div>
          <Button type="submit" disabled={loading || !username.trim()}>
            {loading && <Loader2Icon className="animate-spin" />}
            Send request
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}
