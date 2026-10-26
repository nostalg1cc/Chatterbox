// Shared by the main chat body and the desktop HUD message bubbles.
export const MESSAGE_BODY_INITIAL = { opacity: 0, x: -30, scale: 0.92 };
export const MESSAGE_BODY_ANIMATE = { opacity: [0, 1, 1], x: [-30, 3, 0], scale: [0.92, 1.01, 1] };
export const MESSAGE_BODY_TRANSITION = { duration: 0.36, delay: 0.215, times: [0, 0.7, 1], ease: [0.22, 0.78, 0.3, 1] as [number, number, number, number] };
