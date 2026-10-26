/** Wide captures fill the preview by cropping, preserving their proportions. */
export function shouldFillScreenPreview(width: number, height: number): boolean {
  return Number.isFinite(width) && Number.isFinite(height) && height > 0 && width / height > 16 / 9 + 0.001;
}
