import { useEffect, useState } from "react";
import { create } from "zustand";
import type { RealtimeChannel } from "@supabase/supabase-js";
import { supabase } from "@/lib/supabase";

type PresenceEntry = { activeAt: number };
interface PresenceState { online: Record<string, PresenceEntry>; join: (userId: string) => () => void; }
let channel: RealtimeChannel | null = null;
let lastTrackedAt = 0;
const ACTIVE_EVENT_THROTTLE_MS = 30_000;
function activeTimestamp(entry: unknown): number { if (!entry || typeof entry !== "object") return Date.now(); const data=entry as {active_at?:unknown;online_at?:unknown}; const raw=typeof data.active_at==="string"?data.active_at:typeof data.online_at==="string"?data.online_at:""; const parsed=Date.parse(raw); return Number.isFinite(parsed)?parsed:Date.now(); }
export const usePresence = create<PresenceState>()((set) => ({ online:{}, join:(userId) => { if(channel) supabase.removeChannel(channel); channel=supabase.channel("online",{config:{presence:{key:userId}}}); const trackActive=()=>{if(!channel||Date.now()-lastTrackedAt<ACTIVE_EVENT_THROTTLE_MS)return;lastTrackedAt=Date.now();void channel.track({online_at:new Date().toISOString(),active_at:new Date().toISOString()});}; const onVisible=()=>{if(document.visibilityState==="visible")trackActive();}; channel.on("presence",{event:"sync"},()=>{const state=channel?.presenceState()??{};const online:Record<string,PresenceEntry>={};for(const [key,entries] of Object.entries(state)){const list=Array.isArray(entries)?entries:[];online[key]={activeAt:Math.max(...list.map(activeTimestamp),0)||Date.now()};}set({online});}).subscribe((status)=>{if(status==="SUBSCRIBED"){lastTrackedAt=0;trackActive();}}); const events=["pointerdown","pointermove","keydown","scroll","focus"];events.forEach((event)=>window.addEventListener(event,trackActive,{passive:true}));document.addEventListener("visibilitychange",onVisible);return()=>{events.forEach((event)=>window.removeEventListener(event,trackActive));document.removeEventListener("visibilitychange",onVisible);if(channel){supabase.removeChannel(channel);channel=null;}set({online:{}});}; }}));
export function usePresenceStatus(userId: string | undefined): "online" | "away" | "offline" { const entry=usePresence((state)=>userId?state.online[userId]:undefined); const [,setTick]=useState(0); useEffect(()=>{const id=window.setInterval(()=>setTick((value)=>value+1),60_000);return()=>window.clearInterval(id);},[]); if(!entry)return "offline";return Date.now()-entry.activeAt>10*60_000?"away":"online"; }
export function useIsOnline(userId: string | undefined): boolean { return usePresence((state)=>userId?Boolean(state.online[userId]):false); }
