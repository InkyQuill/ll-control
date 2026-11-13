/** Normalized coordinates in the displayed source, matching the native crop DTO. */
export interface VideoCrop {
  x: number;
  y: number;
  width: number;
  height: number;
}
export type CropFields = { [K in keyof VideoCrop]: number | undefined };
/** Editor fields are percentages; invalid/empty values must never reach IPC. */
export function normalizedCrop(fields: CropFields): VideoCrop | null {
  const { x, y, width, height } = fields;
  if (
    x === undefined ||
    y === undefined ||
    width === undefined ||
    height === undefined ||
    ![x, y, width, height].every(Number.isFinite) ||
    x < 0 ||
    y < 0 ||
    width <= 0 ||
    height <= 0 ||
    x + width > 100 ||
    y + height > 100
  )
    return null;
  return {
    x: x / 100,
    y: y / 100,
    width: Math.min(width / 100, 1 - x / 100),
    height: Math.min(height / 100, 1 - y / 100),
  };
}
