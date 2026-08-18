// Builds the same feTurbulence noise SVG used by .stage::before and
// .v3-call-glow::after (see styles.css), parameterized so the Appearance
// tab's grain size/intensity sliders can regenerate it instead of the
// original hardcoded data URI.
export function grainDataUrl(baseFrequency: number): string {
  // A literal # here, not %23 - encodeURIComponent below does the one and
  // only encoding pass. Writing %23 directly would get re-encoded to %2523,
  // corrupting the filter reference so feTurbulence silently never applies
  // (this was the original bug - the grain was completely invisible).
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='240' height='240'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='${baseFrequency}' numOctaves='2' stitchTiles='stitch'/></filter><rect width='100%' height='100%' filter='url(#n)'/></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

// 0 (finest grain) - 100 (coarsest/largest grain) maps inversely onto the
// SVG filter's baseFrequency, where lower frequency reads as bigger blobs.
export function grainSizeToBaseFrequency(size: number): number {
  const clamped = Math.min(100, Math.max(0, size));
  return 0.9 - (clamped / 100) * 0.85;
}
