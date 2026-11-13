import type { Device } from './api';
export interface DisplayGeometry {
  readonly shape: 'square' | 'round';
  readonly width: number;
  readonly height: number;
}
export interface DeviceCapabilities {
  display: DisplayGeometry | null;
  lighting: boolean;
  cooling: boolean;
}
const none: DeviceCapabilities = { display: null, lighting: false, cooling: false };
/** The registry describes implemented editors, not every feature advertised by a vendor. */
export function deviceCapabilities(device: Device | undefined): DeviceCapabilities {
  if (!device) return none;
  if (device.family === 'galahad2-lcdv2') {
    return { display: { shape: 'square', width: 480, height: 480 }, lighting: true, cooling: true };
  }
  return none;
}
