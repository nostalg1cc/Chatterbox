import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  CameraIcon,
  HardDriveIcon,
  HeadphonesIcon,
  KeyboardIcon,
  Loader2Icon,
  Music2Icon,
  LogOutIcon,
  MessageSquareIcon,
  MicIcon,
  PlusIcon,
  SettingsIcon,
  Trash2Icon,
  UserRoundIcon,
  Volume2Icon,
} from "lucide-react";
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
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import {
  clearLocalMediaCache,
  mediaCacheStats,
  type MediaCacheStats,
} from "@/lib/media-cache";
import { playAppSound } from "@/lib/app-sounds";
import { eventKeybind, keybindLabel } from "@/lib/keybinds";
import { formattedBytes, prepareAnimatedAvatar, prepareAvatar } from "@/lib/media";
import { NAME_COLOR_OPTIONS, nameColorClass } from "@/lib/name-colors";
import { AVATAR_DECORATIONS } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import type { NameColor } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { usePreferences, type KeybindPreferences } from "@/stores/preferences";
import { useProfiles } from "@/stores/profiles";
import { useSoundboard } from "@/stores/soundboard";
import { useVoice } from "@/stores/voice";

export function SettingsDialog() {
  const profile = useAuth((state) => state.profile);
  const email = useAuth((state) => state.email);
  const userId = useAuth((state) => state.userId);
  const preferences = usePreferences();
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState("general");
  const [displayName, setDisplayName] = useState("");
  const [nameColor, setNameColor] = useState<NameColor>("default");
  const [decoration, setDecoration] = useState<string | null>(null);
  const [nameDecoration, setNameDecoration] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [avatarBusy, setAvatarBusy] = useState(false);
  const [clearingCache, setClearingCache] = useState(false);
  const [cacheStats, setCacheStats] = useState<MediaCacheStats | null>(null);
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const avatarInput = useRef<HTMLInputElement>(null);
  const soundInput = useRef<HTMLInputElement>(null);
  const [soundName, setSoundName] = useState("");
  const sounds = useSoundboard((state) => state.sounds);
  const soundUploading = useSoundboard((state) => state.uploading);

  const refreshCacheStats = async () => {
    if (!userId) return;
    try {
      setCacheStats(await mediaCacheStats(userId));
    } catch (error) {
      console.warn("Could not read local media cache stats", error);
    }
  };

  useEffect(() => {
    if (!open) return;
    void refreshCacheStats();
    if (navigator.mediaDevices?.enumerateDevices) {
      void navigator.mediaDevices
        .enumerateDevices()
        .then(setDevices)
        .catch(() => setDevices([]));
    }
  }, [open, userId]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    const name = displayName.trim();
    if (!name) return;
    setSaving(true);
    try {
      await useAuth.getState().updateGeneralSettings(name, nameColor, decoration, nameDecoration);
      const updated = useAuth.getState().profile;
      if (updated) useProfiles.getState().put([updated]);
      toast.success("Profile updated.");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Couldn't save.");
    } finally {
      setSaving(false);
    }
  };

  const uploadAvatar = async (file: File | undefined) => {
    if (!file || !userId) return;
    setAvatarBusy(true);
    try {
      const isGif = file.type === "image/gif";
      const animatedGif = isGif ? await prepareAnimatedAvatar(file) : null;
      const cover = await prepareAvatar(file);
      const path = userId + "/avatar.webp";
      const animatedPath = userId + "/avatar.gif";
      const { error: coverError } = await supabase.storage.from("avatars").upload(path, cover, {
        upsert: true,
        contentType: "image/webp",
        cacheControl: "3600",
      });
      if (coverError) throw new Error(coverError.message);
      if (isGif) {
        const { error: gifError } = await supabase.storage.from("avatars").upload(animatedPath, animatedGif!, {
          upsert: true,
          contentType: "image/gif",
          cacheControl: "3600",
        });
        if (gifError) throw new Error(gifError.message);
      } else {
        await supabase.storage.from("avatars").remove([animatedPath]);
      }
      await useAuth.getState().updateAvatar(path, isGif ? animatedPath : null);
      const updated = useAuth.getState().profile;
      if (updated) useProfiles.getState().put([updated]);
      toast.success(isGif ? "Animated avatar updated." : "Avatar updated.");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Couldn't update your avatar.");
    } finally {
      setAvatarBusy(false);
      if (avatarInput.current) avatarInput.current.value = "";
    }
  };

  const uploadSound = async (file: File | undefined) => {
    if (!file) return;
    const name = soundName.trim() || file.name.replace(/\.[^.]+$/, "").slice(0, 32);
    try {
      await useSoundboard.getState().upload(file, name);
      setSoundName("");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Couldn't add the sound.");
    } finally {
      if (soundInput.current) soundInput.current.value = "";
    }
  };

  const clearCache = async () => {
    if (!userId) return;
    setClearingCache(true);
    try {
      await clearLocalMediaCache(userId);
      await refreshCacheStats();
      toast.success("Local media cache cleared.");
    } catch {
      toast.error("Couldn't clear the local media cache.");
    } finally {
      setClearingCache(false);
    }
  };

  const microphones = devices.filter((device) => device.kind === "audioinput");
  const speakers = devices.filter((device) => device.kind === "audiooutput");
  const profileChanged =
    displayName.trim() !== (profile?.display_name ?? "") ||
    nameColor !== (profile?.name_color ?? "default") || decoration !== (profile?.avatar_decoration ?? null) || nameDecoration !== (profile?.name_decoration ?? null);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) {
          setTab("general");
          setDisplayName(profile?.display_name ?? "");
          setNameColor(profile?.name_color ?? "default");
          setDecoration(profile?.avatar_decoration ?? null);
          setNameDecoration(profile?.name_decoration ?? null);
          void useSoundboard.getState().load();
        }
      }}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Settings"
              className="text-muted-foreground"
            >
              <SettingsIcon />
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent>Settings</TooltipContent>
      </Tooltip>

      <DialogContent overlayClassName="bg-black/70 supports-backdrop-filter:backdrop-blur-none" className="flex h-[calc(100vh-48px)] max-h-[760px] min-h-0 flex-col overflow-hidden rounded-lg border-white/[0.18] p-0 sm:max-w-[940px]">
        <DialogHeader className="sr-only">
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>Profile, chat, and voice preferences.</DialogDescription>
        </DialogHeader>

        <Tabs
          value={tab}
          onValueChange={setTab}
          orientation="vertical"
          className="flex h-full min-h-0 flex-1 flex-row gap-0"
        >
          <aside className="flex w-56 shrink-0 flex-col border-r border-white/[0.13] bg-[#151515] p-4">
            <p className="px-2 pb-2 text-[10px] font-semibold tracking-[0.12em] text-muted-foreground uppercase">
              User settings
            </p>
            <TabsList
              variant="line"
              className="flex w-full flex-col items-stretch gap-1 bg-transparent p-0"
            >
              <SettingsTab value="general" icon={<UserRoundIcon />}>
                My Account
              </SettingsTab>
              <SettingsTab value="chat" icon={<MessageSquareIcon />}>
                Chat
              </SettingsTab>
              <SettingsTab value="voice" icon={<HeadphonesIcon />}>
                Voice & Video
              </SettingsTab>
              <SettingsTab value="keybinds" icon={<KeyboardIcon />}>
                Keybinds
              </SettingsTab>
              <SettingsTab value="soundboard" icon={<Music2Icon />}>
                Soundboard
              </SettingsTab>
            </TabsList>

            <div className="mt-auto">
              <Separator className="mb-2" />
              <Button
                variant="ghost"
                className="w-full justify-start text-muted-foreground hover:text-destructive"
                onClick={() => {
                  setOpen(false);
                  void (async () => {
                    await useVoice.getState().leave();
                    await useAuth.getState().signOut();
                  })();
                }}
              >
                <LogOutIcon />
                Sign out
              </Button>
            </div>
          </aside>

          <div className="flex min-w-0 min-h-0 flex-1 flex-col overflow-hidden">
            <TabsContent value="general" className="mt-0 hidden min-h-0 flex-1 overflow-y-auto p-6 pr-8 data-[state=active]:block">
              <section className="space-y-5">
                <div>
                  <h2 className="text-lg font-semibold">My Account</h2>
                  <p className="mt-1 text-xs text-muted-foreground">
                    Changes are pushed live to friends and active conversations.
                  </p>
                </div>

                <div className="flex items-center gap-3 rounded-xl border border-white/[0.14] bg-card p-4">
                  <UserAvatar profile={profile} size="lg" />
                  <div className="min-w-0 flex-1">
                    <p className={cn("truncate text-sm font-medium", nameColorClass(nameColor))}>
                      {displayName.trim() || profile?.display_name}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      512px WebP, compressed locally
                    </p>
                  </div>
                  <input
                    ref={avatarInput}
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(event) => void uploadAvatar(event.target.files?.[0])}
                  />
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={avatarBusy}
                    onClick={() => avatarInput.current?.click()}
                  >
                    {avatarBusy ? <Loader2Icon className="animate-spin" /> : <CameraIcon />}
                    Change
                  </Button>
                </div>

                <div className="rounded-xl border border-white/[0.14] bg-card">
                  <ToggleRow
                    title="Acrylic window material"
                    description="Off uses Windows 11 Mica; on uses Acrylic for the full app backdrop."
                    checked={preferences.windowMaterial === "acrylic"}
                    onChange={(value) =>
                      preferences.setPreference("windowMaterial", value ? "acrylic" : "mica")
                    }
                  />
                  {preferences.windowMaterial === "acrylic" && (
                    <div className="border-t border-white/[0.10] px-4 py-3.5">
                      <div className="mb-2 flex items-center justify-between gap-3">
                        <div>
                          <p className="text-sm font-medium">Acrylic dim</p>
                          <p className="text-xs text-muted-foreground">
                            Adds a black tint across the window backdrop.
                          </p>
                        </div>
                        <span className="text-xs tabular-nums text-muted-foreground">
                          {preferences.acrylicDim}%
                        </span>
                      </div>
                      <input
                        type="range"
                        min={0}
                        max={100}
                        step={1}
                        value={preferences.acrylicDim}
                        aria-label="Acrylic dim"
                        className="h-1.5 w-full cursor-pointer accent-white"
                        onChange={(event) =>
                          preferences.setPreference("acrylicDim", Number(event.target.value))
                        }
                      />
                    </div>
                  )}
                </div>
                <div className="space-y-2 rounded-xl border border-white/[0.14] bg-card p-4">
                  <Label>Avatar decoration</Label>
                  <div className="grid grid-cols-5 gap-2">
                    <button type="button" onClick={() => setDecoration(null)} className={cn("rounded-md border p-1 text-[10px]", !decoration && "border-white/50")}>None</button>
                    {AVATAR_DECORATIONS.map(([id, label]) => <button key={id} type="button" title={label} onClick={() => setDecoration(id)} className={cn("relative aspect-square overflow-visible rounded-md border", decoration === id && "border-white/60 bg-white/10")}><img src={`/decorations/${id}.png`} alt={label} className="absolute -inset-2 size-[calc(100%+1rem)] max-w-none" /></button>)}
                  </div>
                  <p className="text-xs text-muted-foreground">Animated in your user panel and the partner header; static elsewhere until hover.</p>
                </div>
<div className="space-y-2 rounded-xl border border-white/[0.14] bg-card p-4"><Label>Name effect</Label><div className="grid grid-cols-4 gap-2">{["fuzzy","sparkles","resize","bouncy","wavy","gradient","glitch","particle"].map((effect) => <button key={effect} type="button" onClick={() => setNameDecoration(effect)} className={cn("rounded-md border px-2 py-1 text-xs capitalize", nameDecoration === effect && "border-white/60 bg-white/10")}>{effect}</button>)}<button type="button" onClick={() => setNameDecoration(null)} className="rounded-md border px-2 py-1 text-xs">None</button></div></div>                <form onSubmit={save} className="space-y-4">
                  <div className="space-y-1.5">
                    <Label htmlFor="settings-displayname">Display name</Label>
                    <Input
                      id="settings-displayname"
                      value={displayName}
                      maxLength={50}
                      onChange={(event) => setDisplayName(event.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label>Chat name color</Label>
                    <div className="grid grid-cols-5 gap-2">
                      {NAME_COLOR_OPTIONS.map((option) => (
                        <button
                          key={option.value}
                          type="button"
                          aria-label={option.label}
                          aria-pressed={nameColor === option.value}
                          className={cn(
                            "flex h-9 items-center justify-center rounded-lg border border-white/[0.12] bg-muted/35 transition-colors hover:bg-muted/75",
                            nameColor === option.value &&
                              "border-white/[0.28] bg-muted ring-2 ring-white/[0.12]"
                          )}
                          onClick={() => setNameColor(option.value)}
                        >
                          <span className={cn("size-3.5 rounded-full", option.swatch)} />
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <Label htmlFor="settings-username">Username</Label>
                      <Input
                        id="settings-username"
                        value={"@" + (profile?.username ?? "")}
                        disabled
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="settings-email">Email</Label>
                      <Input id="settings-email" value={email ?? ""} disabled />
                    </div>
                  </div>

                  <Button
                    type="submit"
                    variant="secondary"
                    disabled={saving || !displayName.trim() || !profileChanged}
                  >
                    {saving && <Loader2Icon className="animate-spin" />}
                    Save changes
                  </Button>
                </form>
              </section>
            </TabsContent>

            <TabsContent value="chat" className="mt-0 hidden min-h-0 flex-1 overflow-y-auto p-6 pr-8 data-[state=active]:block">
              <section className="space-y-5">
                <div>
                  <h2 className="text-lg font-semibold">Chat</h2>
                  <p className="mt-1 text-xs text-muted-foreground">
                    Message behavior and local attachment storage.
                  </p>
                </div>

                <div className="rounded-xl border border-white/[0.14] bg-card p-4">
                  <div className="flex items-start gap-3">
                    <HardDriveIcon className="mt-0.5 size-5 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium">30-day local media cache</p>
                      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                        Compressed chat images and videos remain on this device for 30 days,
                        even after the temporary server copy is purged.
                      </p>
                      <p className="mt-2 text-xs text-foreground/80">
                        {cacheStats
                          ? formattedBytes(cacheStats.bytes) +
                            " in " +
                            cacheStats.entries +
                            " files / " +
                            formattedBytes(cacheStats.limitBytes) +
                            " limit"
                          : "Calculating local usage..."}
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={clearingCache || !cacheStats?.entries}
                      onClick={() => void clearCache()}
                    >
                      {clearingCache ? (
                        <Loader2Icon className="animate-spin" />
                      ) : (
                        <Trash2Icon />
                      )}
                      Clear
                    </Button>
                  </div>
                </div>

                <div className="divide-y divide-white/[0.10] rounded-xl border border-white/[0.14] bg-card">
                  <ToggleRow
                    title="Enter to send"
                    description="When off, use Ctrl+Enter to send."
                    checked={preferences.enterToSend}
                    onChange={(value) => preferences.setPreference("enterToSend", value)}
                  />
                  <ToggleRow
                    title="Compact message spacing"
                    description="Fit more conversation history on screen."
                    checked={preferences.compactMessages}
                    onChange={(value) => preferences.setPreference("compactMessages", value)}
                  />
                  <ToggleRow
                    title="Show media previews"
                    description="Render images and videos directly in the conversation."
                    checked={preferences.showMediaPreviews}
                    onChange={(value) => preferences.setPreference("showMediaPreviews", value)}
                  />
                </div>
              </section>
            </TabsContent>

            <TabsContent value="voice" className="mt-0 hidden min-h-0 flex-1 overflow-y-auto p-6 pr-8 data-[state=active]:block">
              <section className="space-y-5">
                <div>
                  <h2 className="text-lg font-semibold">Voice & Video</h2>
                  <p className="mt-1 text-xs text-muted-foreground">
                    Device changes apply immediately while you are connected to voice.
                  </p>
                </div>

                <div className="space-y-4 rounded-xl border border-white/[0.14] bg-card p-4">
                  <DeviceSelect
                    id="audio-input"
                    label="Input device"
                    icon={<MicIcon />}
                    value={preferences.inputDeviceId}
                    devices={microphones}
                    fallback="Default microphone"
                    onChange={(value) => preferences.setPreference("inputDeviceId", value)}
                  />
                  <VolumeSetting
                    label="Input volume"
                    value={preferences.inputVolume}
                    onChange={(value) => preferences.setPreference("inputVolume", value)}
                  />
                  <Separator />
                  <DeviceSelect
                    id="audio-output"
                    label="Output device"
                    icon={<Volume2Icon />}
                    value={preferences.outputDeviceId}
                    devices={speakers}
                    fallback="Default speakers"
                    onChange={(value) => preferences.setPreference("outputDeviceId", value)}
                  />
                  <VolumeSetting
                    label="Output volume"
                    value={preferences.outputVolume}
                    onChange={(value) => preferences.setPreference("outputVolume", value)}
                  />
                  <div className="flex items-center justify-between rounded-md border border-white/[0.11] bg-muted/20 px-3 py-2">
                    <div>
                      <p className="text-xs font-medium">Output test</p>
                      <p className="text-[11px] text-muted-foreground">
                        Plays through the selected device, with Windows default as fallback.
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => playAppSound("notification_single", true)}
                    >
                      Test sound
                    </Button>
                  </div>
                </div>

                <div className="divide-y divide-white/[0.10] rounded-lg border border-white/[0.14] bg-card">
                  <ToggleRow
                    title="Interface sounds"
                    description="Play join, leave, mute, and deafen feedback."
                    checked={preferences.interfaceSounds}
                    onChange={(value) => preferences.setPreference("interfaceSounds", value)}
                  />
                  <div className="p-4">
                    <VolumeSetting
                      label="Interface sound volume"
                      value={preferences.interfaceSoundVolume}
                      onChange={(value) => preferences.setPreference("interfaceSoundVolume", value)}
                    />
                  </div>
                  <div className="p-4">
                    <VolumeSetting
                      label="Soundboard volume"
                      value={preferences.soundboardVolume}
                      onChange={(value) => preferences.setPreference("soundboardVolume", value)}
                    />
                  </div>
                </div>
                <p className="rounded-lg border border-dashed border-white/[0.13] px-3 py-2 text-xs text-muted-foreground">
                  Microphone permission is requested only when you join voice. Device names
                  appear after the first join; unsupported output routing uses Windows default.
                </p>
              </section>
            </TabsContent>
            <TabsContent value="keybinds" className="mt-0 hidden min-h-0 flex-1 overflow-y-auto p-8 data-[state=active]:block">
              <section className="mx-auto max-w-2xl space-y-5">
                <SettingsHeading title="Keybinds" description="Shortcuts work anywhere inside Dislight. Recording a duplicate replaces the old binding." />
                <button
                  type="button"
                  role="switch"
                  aria-checked={preferences.globalVoiceShortcuts}
                  onClick={() => preferences.setPreference("globalVoiceShortcuts", !preferences.globalVoiceShortcuts)}
                  className="flex w-full items-center justify-between rounded-lg border border-white/[0.14] bg-card p-4 text-left transition-colors hover:bg-muted/40"
                >
                  <span>
                    <span className="block text-sm font-medium">Allow global mute & deafen</span>
                    <span className="mt-1 block text-xs text-muted-foreground">Lets these two shortcuts work while Dislight is unfocused. Other shortcuts stay app-only.</span>
                  </span>
                  <span className={cn("relative h-5 w-9 rounded-full transition-colors", preferences.globalVoiceShortcuts ? "bg-emerald-500" : "bg-muted")}>
                    <span className={cn("absolute top-0.5 size-4 rounded-full bg-white shadow transition-transform", preferences.globalVoiceShortcuts ? "translate-x-4" : "translate-x-0.5")} />
                  </span>
                </button>
                <div className="divide-y divide-white/[0.10] rounded-lg border border-white/[0.14] bg-card">
                  {(Object.keys(preferences.keybinds) as (keyof KeybindPreferences)[]).map((key) => (
                    <KeybindRow
                      key={key}
                      action={key}
                      value={preferences.keybinds[key]}
                      onChange={(value) => {
                        const next = { ...preferences.keybinds };
                        for (const existing of Object.keys(next) as (keyof KeybindPreferences)[]) {
                          if (next[existing] === value) next[existing] = "";
                        }
                        next[key] = value;
                        preferences.setPreference("keybinds", next);
                      }}
                    />
                  ))}
                </div>
              </section>
            </TabsContent>

            <TabsContent value="soundboard" className="mt-0 hidden min-h-0 flex-1 overflow-y-auto p-8 data-[state=active]:block">
              <section className="mx-auto max-w-2xl space-y-5">
                <SettingsHeading title="Soundboard" description="Clips are trimmed, normalized, and converted locally to 48 kHz mono Opus at 96 kbps." />
                <div className="rounded-lg border border-white/[0.14] bg-card p-4">
                  <div className="flex gap-2">
                    <Input value={soundName} maxLength={32} placeholder="Optional sound name" onChange={(event) => setSoundName(event.target.value)} />
                    <input ref={soundInput} className="hidden" type="file" accept="audio/*" onChange={(event) => void uploadSound(event.target.files?.[0])} />
                    <Button variant="secondary" disabled={soundUploading || sounds.length >= 24} onClick={() => soundInput.current?.click()}>
                      {soundUploading ? <Loader2Icon className="animate-spin" /> : <PlusIcon />}
                      Add sound
                    </Button>
                  </div>
                  <p className="mt-2 text-[11px] text-muted-foreground">15 seconds max ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· 512 KiB prepared max ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· 24 clips ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· 16 MiB per account</p>
                </div>
                <div className="space-y-2">
                  {sounds.map((sound) => (
                    <div key={sound.id} className="flex items-center gap-3 rounded-md border border-white/[0.13] bg-card px-3 py-2.5">
                      <Music2Icon className="size-4 text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium">{sound.name}</p>
                        <p className="text-[11px] text-muted-foreground">{(sound.duration_ms / 1000).toFixed(1)}s ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· {formattedBytes(sound.size_bytes)}</p>
                      </div>
                      <Button variant="ghost" size="icon-sm" aria-label={"Delete " + sound.name} className="text-muted-foreground hover:text-destructive" onClick={() => void useSoundboard.getState().remove(sound.id)}>
                        <Trash2Icon />
                      </Button>
                    </div>
                  ))}
                  {!sounds.length && <p className="rounded-lg border border-dashed border-white/[0.13] p-8 text-center text-xs text-muted-foreground">No sounds yet. Add a short clip, then play it from the chat header while in voice.</p>}
                </div>
              </section>
            </TabsContent>
          </div>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

const KEYBIND_LABELS: Record<keyof KeybindPreferences, string> = {
  toggleMute: "Toggle mute",
  toggleDeafen: "Toggle deafen",
  leaveVoice: "Leave voice",
  toggleScreenShare: "Toggle screen share",
  openSoundboard: "Open soundboard",
};

function SettingsHeading({ title, description }: { title: string; description: string }) {
  return (
    <div>
      <h2 className="text-xl font-semibold">{title}</h2>
      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{description}</p>
    </div>
  );
}

function KeybindRow({
  action,
  value,
  onChange,
}: {
  action: keyof KeybindPreferences;
  value: string;
  onChange: (value: string) => void;
}) {
  const [recording, setRecording] = useState(false);

  useEffect(() => {
    if (!recording) return;
    const capture = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.key === "Escape") {
        setRecording(false);
        return;
      }
      if (event.key === "Backspace" || event.key === "Delete") {
        onChange("");
        setRecording(false);
        return;
      }
      const binding = eventKeybind(event);
      if (!binding) return;
      onChange(binding);
      setRecording(false);
    };
    window.addEventListener("keydown", capture, true);
    return () => window.removeEventListener("keydown", capture, true);
  }, [onChange, recording]);

  return (
    <div className="flex items-center gap-4 p-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium">{KEYBIND_LABELS[action]}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">
          {action === "openSoundboard" ? "Available while connected to voice." : "Press Escape to cancel or Backspace to clear."}
        </p>
      </div>
      <Button
        type="button"
        variant={recording ? "secondary" : "outline"}
        className="min-w-40 font-normal"
        onClick={() => setRecording(true)}
      >
        {recording ? "Press a combinationÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¦" : value ? keybindLabel(value) : "Not set"}
      </Button>
    </div>
  );
}
function SettingsTab({
  value,
  icon,
  children,
}: {
  value: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <TabsTrigger
      value={value}
      className="h-9 w-full justify-start rounded-md px-2.5 data-active:bg-muted/90"
    >
      {icon}
      {children}
    </TabsTrigger>
  );
}

function ToggleRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-4 p-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium">{title}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        className={cn(
          "relative h-5 w-9 shrink-0 rounded-full border transition-colors",
          checked
            ? "border-foreground/35 bg-foreground/80"
            : "border-white/[0.18] bg-muted"
        )}
        onClick={() => onChange(!checked)}
      >
        <span
          className={cn(
            "absolute top-0.5 size-3.5 rounded-full bg-background shadow-sm transition-[left]",
            checked ? "left-[18px]" : "left-0.5"
          )}
        />
      </button>
    </div>
  );
}

function DeviceSelect({
  id,
  label,
  icon,
  value,
  devices,
  fallback,
  onChange,
}: {
  id: string;
  label: string;
  icon: React.ReactNode;
  value: string;
  devices: MediaDeviceInfo[];
  fallback: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id} className="flex items-center gap-1.5">
        {icon}
        {label}
      </Label>
      <select
        id={id}
        value={value}
        className="h-9 w-full rounded-lg border border-white/[0.14] bg-input/40 px-2.5 text-sm outline-none transition-colors hover:bg-input/55 focus:border-white/[0.25]"
        onChange={(event) => onChange(event.target.value)}
      >
        <option value="default">{fallback}</option>
        {devices.map((device, index) => (
          <option key={device.deviceId || label + index} value={device.deviceId}>
            {device.label || label + " " + (index + 1)}
          </option>
        ))}
      </select>
    </div>
  );
}

function VolumeSetting({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>{label}</span>
        <span className="tabular-nums">{value}%</span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={value}
        aria-label={label}
        className="w-full accent-foreground"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </div>
  );
}
