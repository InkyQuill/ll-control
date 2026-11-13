export interface VideoAdjustments {
  contrast: number;
  brightness: number;
}
export function validVideoAmount(value: number | undefined): value is number {
  return value !== undefined && Number.isInteger(value) && value >= 0 && value <= 255;
}
function byte(value: number): number {
  const lower = Math.floor(value),
    fraction = value - lower;
  const rounded = fraction === 0.5 ? lower + (lower % 2) : Math.round(value);
  return Math.max(0, Math.min(255, rounded));
}
/** Separate f32 operations and byte boundaries match the SDK SIMD/Rust kernel. */
export function videoChannelLut({ contrast, brightness }: VideoAdjustments): Uint8Array {
  if (!validVideoAmount(contrast) || !validVideoAmount(brightness))
    throw new RangeError('Video adjustments must be integers in 0..255');
  const f = Math.fround;
  const factor = f(f(f(contrast + 255) * 259) / f(f(259 - contrast) * 255));
  const lightness = f(brightness / 255);
  return Uint8Array.from({ length: 256 }, (_, value) => {
    const contrasted = byte(f(f(f(value - 128) * factor) + 128));
    return byte(f(contrasted + f(f(255 - contrasted) * lightness)));
  });
}
