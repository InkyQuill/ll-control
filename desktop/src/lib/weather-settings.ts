import type { EffectSensor, Theme } from './api';
import { ValidationError } from './validation-error';
export function weatherRangeMaximum(binding: EffectSensor | null | undefined): number {
  return binding?.unit === 'rpm' ? 3600 : 100;
}
export function weatherRangeValid(binding: EffectSensor | null | undefined): boolean {
  return (
    !binding ||
    (Number.isInteger(binding.low) &&
      Number.isInteger(binding.high) &&
      binding.low >= 0 &&
      binding.high <= weatherRangeMaximum(binding) &&
      binding.low <= binding.high)
  );
}
// Keep historical v3 JSON readable in Rust; constrain new GUI edits to the
// LCDV2 controls. Explicit Apply and profile Save must not bypass input bounds.
export function assertWeatherDraft(theme: Theme): void {
  const fireworks = theme.layers.find((layer) => layer.kind === 'fireworks');
  if (fireworks?.effect.dynamic && !fireworks.effect.sensor)
    throw new ValidationError('Select an effect source before saving or applying.');
  if (
    fireworks &&
    (!Number.isInteger(fireworks.effect.intensity) ||
      fireworks.effect.intensity < 1 ||
      fireworks.effect.intensity > 3 ||
      !weatherRangeValid(fireworks.effect.sensor?.binding))
  )
    throw new ValidationError('Check the Fireworks intensity and thresholds.');
  const snow3d = theme.layers.find((layer) => layer.kind === 'snow3d');
  if (
    snow3d &&
    (!Number.isInteger(snow3d.effect.intensity) ||
      snow3d.effect.intensity < 1 ||
      snow3d.effect.intensity > 3)
  )
    throw new ValidationError('Check the Snow3D intensity: 1–3.');
  const meteor = theme.layers.find((layer) => layer.kind === 'meteor');
  if (meteor?.effect.dynamic && !meteor.effect.sensor)
    throw new ValidationError('Select an effect source before saving or applying.');
  if (
    meteor &&
    (!Number.isInteger(meteor.effect.speed) ||
      meteor.effect.speed < 1 ||
      meteor.effect.speed > 3 ||
      !weatherRangeValid(meteor.effect.sensor?.binding))
  )
    throw new ValidationError('Check the MeteorShower speed and thresholds.');
  const weather = theme.layers.find((layer) => layer.kind === 'weather');
  if (weather?.effect.dynamic && !weather.effect.sensor)
    throw new ValidationError('Select an effect source before saving or applying.');
  if (weather && !weatherRangeValid(weather.effect.sensor)) {
    throw new ValidationError(
      'Check the effect thresholds: 0–100 for °C/% and 0–3600 for RPM; the lower threshold must not exceed the upper threshold.',
    );
  }
}

// These identities are daemon-classified. Labels, ordering and GPU percentages
// cannot identify the Windows CPU Load role.
export function initialWeatherCpuLoad(
  state: import('./api').SensorDefaultsState,
): EffectSensor | null {
  const selected = state.config.cpu.load;
  const candidates = [
    ...new Set(
      state.candidates
        .filter(
          (candidate) =>
            candidate.role === 'cpu_load' &&
            candidate.gpu_id === null &&
            candidate.reading.unit === 'percent',
        )
        .map((candidate) => candidate.reading.id),
    ),
  ];
  const id = selected ?? (candidates.length === 1 ? candidates[0] : null);
  return id ? { sensor_id: id, unit: 'percent', low: 30, high: 60 } : null;
}
