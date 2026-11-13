import { t } from './i18n.svelte';
import type { Sensor } from './api';
export const windowsRoles = [
  'CPULoad',
  'CPUTemperature',
  'GPULoad',
  'GPUTemperature',
  'PumpRPM',
  'CoolantTemp',
] as const;
export type WindowsRole = (typeof windowsRoles)[number];
export type WindowsUnit = 'percent' | 'celsius' | 'rpm';
export interface WindowsOptions {
  record: number;
  bindings: Record<WindowsRole, { sensor_id: string; unit: WindowsUnit }>;
}
export type WindowsIds = Record<WindowsRole, string>;
export const emptyWindowsIds = (): WindowsIds => ({
  CPULoad: '',
  CPUTemperature: '',
  GPULoad: '',
  GPUTemperature: '',
  PumpRPM: '',
  CoolantTemp: '',
});
export const windowsUnit = (role: WindowsRole): WindowsUnit =>
  role === 'PumpRPM' ? 'rpm' : role === 'CPULoad' || role === 'GPULoad' ? 'percent' : 'celsius';
export const windowsUnitLabel = (role: WindowsRole) =>
  windowsUnit(role) === 'celsius' ? '°C' : windowsUnit(role) === 'percent' ? '%' : 'RPM';
export const windowsRoleLabel = (role: WindowsRole) =>
  ({
    CPULoad: () => t('CPU · Загрузка'),
    CPUTemperature: () => t('CPU · Температура'),
    GPULoad: () => t('GPU · Загрузка'),
    GPUTemperature: () => t('GPU · Температура'),
    PumpRPM: () => t('Помпа'),
    CoolantTemp: () => t('Температура жидкости'),
  })[role]();
export const windowsSourceUnverified = (role: WindowsRole, ids: WindowsIds, sensors: Sensor[]) =>
  !!ids[role] &&
  !sensors.some(
    (s) =>
      s.id === ids[role] &&
      s.unit === windowsUnit(role) &&
      s.status === 'valid' &&
      s.age_ms !== null &&
      s.age_ms <= 3000,
  );
export function windowsBindingInvalid(
  role: WindowsRole,
  ids: WindowsIds,
  sensors: Sensor[],
): boolean {
  const id = ids[role];
  return (
    !!id &&
    (!id.trim() ||
      new TextEncoder().encode(id).length > 1024 ||
      /\p{Cc}/u.test(id) ||
      windowsRoles.some((other) => other !== role && ids[other] === id) ||
      sensors.some((s) => s.id === id && s.unit !== windowsUnit(role)))
  );
}
export function windowsOptions(
  record: string,
  ids: WindowsIds,
  sensors: Sensor[],
  records: number[],
): WindowsOptions | null {
  const index = Number(record);
  if (record === '' || !Number.isInteger(index) || !records.includes(index)) return null;
  const used = new Set<string>();
  const bindings = {} as WindowsOptions['bindings'];
  for (const role of windowsRoles) {
    const id = ids[role],
      unit = windowsUnit(role);
    if (
      !id.trim() ||
      new TextEncoder().encode(id).length > 1024 ||
      /\p{Cc}/u.test(id) ||
      used.has(id) ||
      sensors.some((s) => s.id === id && s.unit !== unit)
    )
      return null;
    used.add(id);
    bindings[role] = { sensor_id: id, unit };
  }
  return { record: index, bindings };
}
export const windowsOptionsKey = (options: WindowsOptions | null | undefined) =>
  options
    ? JSON.stringify([
        options.record,
        ...windowsRoles.map((role) => [
          options.bindings[role]?.sensor_id,
          options.bindings[role]?.unit,
        ]),
      ])
    : '';
export interface WindowsColor {
  r: number;
  g: number;
  b: number;
}
export interface WindowsLightingMode {
  mode: string;
  color: WindowsColor;
  additional_colors?: WindowsColor[];
  brightness_percent: number;
  speed_percent: number;
  direction: string;
  dynamic_mode?: boolean;
}
export interface WindowsDynamicBinding {
  sensor_id: string;
  unit: WindowsUnit;
  low_value: number;
  high_value: number;
  low_colors: WindowsColor[];
  high_colors: WindowsColor[];
  low_speed_percent: number | null;
  high_speed_percent: number | null;
  brightness_percent?: number;
  direction?: string;
}
export interface WindowsLightingReview {
  fan_topology: { sync_to_pump: boolean; size: '280' | '360'; manual_led_count: number };
  fan_modes: WindowsLightingMode[];
  pump_modes: WindowsLightingMode[];
  dynamic_rgb_profiles: {
    modes: { mode: string; enabled: boolean; bindings: WindowsDynamicBinding[] }[];
  };
}
