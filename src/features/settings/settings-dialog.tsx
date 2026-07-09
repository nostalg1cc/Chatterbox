import { useState } from "react";
import { toast } from "sonner";
import { Loader2Icon, LogOutIcon, SettingsIcon } from "lucide-react";
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
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useAuth } from "@/stores/auth";

export function SettingsDialog() {
  const profile = useAuth((s) => s.profile);
  const email = useAuth((s) => s.email);
  const [open, setOpen] = useState(false);
  const [displayName, setDisplayName] = useState("");
  const [saving, setSaving] = useState(false);

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    const name = displayName.trim();
    if (!name || name === profile?.display_name) return;
    setSaving(true);
    try {
      await useAuth.getState().updateDisplayName(name);
      toast.success("Display name updated.");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't save.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) setDisplayName(profile?.display_name ?? "");
      }}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label="Settings"
              className="text-muted-foreground">
              <SettingsIcon />
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent>Settings</TooltipContent>
      </Tooltip>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>Your profile and account.</DialogDescription>
        </DialogHeader>

        <form onSubmit={save} className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="settings-displayname">Display name</Label>
            <div className="flex gap-2">
              <Input
                id="settings-displayname"
                value={displayName}
                maxLength={50}
                onChange={(e) => setDisplayName(e.target.value)}
              />
              <Button
                type="submit"
                variant="secondary"
                disabled={saving || !displayName.trim() || displayName.trim() === profile?.display_name}
              >
                {saving && <Loader2Icon className="animate-spin" />}
                Save
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="settings-username">Username</Label>
            <Input id="settings-username" value={`@${profile?.username ?? ""}`} disabled />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="settings-email">Email</Label>
            <Input id="settings-email" value={email ?? ""} disabled />
          </div>
        </form>

        <Separator />

        <Button
          variant="destructive"
          onClick={() => void useAuth.getState().signOut()}
        >
          <LogOutIcon />
          Sign out
        </Button>
      </DialogContent>
    </Dialog>
  );
}
