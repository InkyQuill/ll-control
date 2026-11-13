import type { CoolingPolicy } from './api';
import { t } from './i18n.svelte';
import { ValidationError } from './validation-error';

export interface CoolingModeReference {
  id: string;
  revision: string;
  policy: CoolingPolicy;
}
export interface DeviceCoolingMode {
  id: string;
  name: string;
  policy: CoolingPolicy;
  use_default_name?: boolean;
}
export interface DeviceCoolingModes {
  version: 1;
  revision: string;
  fan: DeviceCoolingMode[];
  pump: DeviceCoolingMode[];
  persistence: { pending: boolean; saved: boolean; last_error: string | null };
}
type Target = 'fan' | 'pump';

const factoryNames: Record<string, { name: string; label: () => string; target: Target }> = {
  Quiet: { name: 'Quiet', label: () => t('Quiet'), target: 'fan' },
  StandardSpeed: { name: 'Standard', label: () => t('Standard'), target: 'fan' },
  HighSpeed: { name: 'High speed', label: () => t('High speed'), target: 'fan' },
  FullSpeed: { name: 'Full speed', label: () => t('Full speed'), target: 'fan' },
  UserDefine1: { name: 'Custom 1', label: () => t('Custom 1'), target: 'fan' },
  UserDefine2: { name: 'Custom 2', label: () => t('Custom 2'), target: 'fan' },
  UserDefine3: { name: 'Custom 3', label: () => t('Custom 3'), target: 'fan' },
  UserDefine4: { name: 'Custom 4', label: () => t('Custom 4'), target: 'fan' },
  FixRPM: { name: 'Fixed RPM', label: () => t('Fixed RPM'), target: 'pump' },
  PWMPerformance: { name: 'Performance', label: () => t('Performance'), target: 'pump' },
  PWMQuiet: { name: 'Quiet', label: () => t('Quiet'), target: 'pump' },
};
export const supportsCoolingModeDefault = (id: string, target: Target) =>
  factoryNames[id]?.target === target;
export const defaultCoolingModeName = (id: string) => factoryNames[id]?.name;
export function coolingModeLabel(mode: DeviceCoolingMode): string {
  return mode.use_default_name && factoryNames[mode.id] ? factoryNames[mode.id].label() : mode.name;
}
const bytes = (value: string) => new TextEncoder().encode(value).length;
const object = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
const fields = (value: Record<string, unknown>, expected: string[]) =>
  Object.keys(value).length === expected.length &&
  expected.every((key) => Object.hasOwn(value, key));
const integer = (value: unknown, min: number, max: number): value is number =>
  typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max;

function policy(value: unknown, target: Target): value is CoolingPolicy {
  const p = object(value),
    min = target === 'fan' ? 10 : 20;
  if (!p) return false;
  if (p.mode === 'fixed' || p.mode === 'motherboard') {
    return (
      fields(p, ['mode', 'percent']) &&
      (p.mode !== 'motherboard' || target === 'pump') &&
      integer(p.percent, min, 100)
    );
  }
  if (p.mode === 'fixed_rpm')
    return target === 'pump' && fields(p, ['mode', 'rpm']) && integer(p.rpm, 800, 3600);
  if (
    typeof p.mode !== 'string' ||
    !['curve', 'fan_curve', 'curve_rpm'].includes(p.mode) ||
    !fields(p, ['mode', 'sensor_id', 'points', 'hysteresis_c', 'fallback_percent']) ||
    typeof p.sensor_id !== 'string' ||
    bytes(p.sensor_id) < 1 ||
    bytes(p.sensor_id) > 4096 ||
    typeof p.hysteresis_c !== 'number' ||
    !Number.isFinite(p.hysteresis_c) ||
    p.hysteresis_c < 0 ||
    p.hysteresis_c > 10 ||
    !integer(p.fallback_percent, min, 100) ||
    !Array.isArray(p.points) ||
    p.points.length < 2 ||
    p.points.length > 16 ||
    (p.mode === 'fan_curve' && target !== 'fan') ||
    (p.mode === 'curve_rpm' && target !== 'pump')
  )
    return false;
  const speedKey = p.mode === 'curve_rpm' ? 'rpm' : 'percent';
  let previousTemperature = -Infinity,
    previousSpeed = -Infinity;
  for (const raw of p.points) {
    const point = object(raw);
    const speed = point?.[speedKey];
    if (
      !point ||
      !fields(point, ['celsius', speedKey]) ||
      typeof point.celsius !== 'number' ||
      !Number.isFinite(point.celsius) ||
      point.celsius < -50 ||
      point.celsius > 150 ||
      (p.mode !== 'curve' && !Number.isInteger(point.celsius)) ||
      !integer(speed, speedKey === 'rpm' ? 800 : min, speedKey === 'rpm' ? 3600 : 100) ||
      point.celsius <= previousTemperature ||
      speed < previousSpeed
    )
      return false;
    previousTemperature = point.celsius;
    previousSpeed = speed;
  }
  return true;
}

/** A failed response must not become an empty bank or an editable draft. */
export function coolingModes(value: unknown): DeviceCoolingModes | null {
  if (value === null) return null;
  const invalid = () =>
    new ValidationError(
      'The daemon returned invalid saved cooling modes. Your draft is preserved.',
    );
  const root = object(value);
  const persistence = object(root?.persistence);
  if (
    !root ||
    !fields(root, ['version', 'revision', 'fan', 'pump', 'persistence']) ||
    root.version !== 1 ||
    typeof root.revision !== 'string' ||
    !/^[0-9a-f]{64}$/.test(root.revision) ||
    !persistence ||
    !fields(persistence, ['pending', 'saved', 'last_error']) ||
    typeof persistence.pending !== 'boolean' ||
    typeof persistence.saved !== 'boolean' ||
    (persistence.last_error !== null && typeof persistence.last_error !== 'string')
  )
    throw invalid();
  const result: DeviceCoolingModes = {
    version: 1,
    revision: root.revision,
    fan: [],
    pump: [],
    persistence: {
      pending: persistence.pending,
      saved: persistence.saved,
      last_error: persistence.last_error,
    },
  };
  for (const target of ['fan', 'pump'] as const) {
    const modes = root[target];
    if (!Array.isArray(modes) || modes.length > 16) throw invalid();
    const ids = new Set<string>();
    for (const raw of modes) {
      const mode = object(raw);
      if (
        !mode ||
        !fields(
          mode,
          Object.hasOwn(mode, 'use_default_name')
            ? ['id', 'name', 'policy', 'use_default_name']
            : ['id', 'name', 'policy'],
        ) ||
        (mode.use_default_name !== undefined && typeof mode.use_default_name !== 'boolean') ||
        (mode.use_default_name === true &&
          (factoryNames[String(mode.id)]?.target !== target ||
            factoryNames[String(mode.id)]?.name !== mode.name)) ||
        typeof mode.id !== 'string' ||
        !/^[A-Za-z0-9_-]{1,32}$/.test(mode.id) ||
        ids.has(mode.id) ||
        typeof mode.name !== 'string' ||
        /^\p{White_Space}*$/u.test(mode.name) ||
        bytes(mode.name) > 128 ||
        /[\p{Cc}]/u.test(mode.name) ||
        !policy(mode.policy, target)
      )
        throw invalid();
      ids.add(mode.id);
      result[target].push({
        id: mode.id,
        name: mode.name,
        policy: mode.policy,
        ...(mode.use_default_name ? { use_default_name: true } : {}),
      });
    }
  }
  return result;
}

/** Compare policy values independently of JSON property insertion order. */
export function coolingPolicyKey(policy: CoolingPolicy): string {
  if ('points' in policy)
    return JSON.stringify([
      policy.mode,
      policy.sensor_id,
      policy.hysteresis_c,
      policy.fallback_percent,
      policy.points.map((p) => [p.celsius, 'rpm' in p ? p.rpm : p.percent]),
    ]);
  return JSON.stringify([policy.mode, 'rpm' in policy ? policy.rpm : policy.percent]);
}
