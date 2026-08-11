import { create } from "zustand";

type LightboxState = {
  src: string | null;
  show: (src: string) => void;
  hide: () => void;
};

export const useLightbox = create<LightboxState>((set) => ({
  src: null,
  show: (src) => set({ src }),
  hide: () => set({ src: null }),
}));
