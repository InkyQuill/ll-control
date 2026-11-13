import type { Theme, Sensor, Style, Layer } from './api';
import { ValidationError } from './validation-error';
export const presets = [
  { id: 'focus', name: 'Focus', description: 'One large reading and a clock', sensors: 1 },
  { id: 'duo', name: 'Duo', description: 'Two readings on one screen', sensors: 2 },
  { id: 'triple', name: 'Triple', description: 'Three readings on one screen', sensors: 3 },
  { id: 'cycle', name: 'Cycle', description: 'Three alternating readings', sensors: 3 },
  { id: 'quad', name: 'Quad', description: 'Four readings in a grid', sensors: 4 },
  {
    id: 'clock',
    name: 'Clock',
    description: 'Date, time and AM/PM with separate colors',
    sensors: 0,
  },
] as const;
export type PresetId = (typeof presets)[number]['id'];
export function presetTheme(id: PresetId, bindings: Sensor[]): Theme {
  const preset = presets.find((p) => p.id === id)!;
  if (bindings.length !== preset.sensors)
    throw new ValidationError('Sensors required for {0}: {1}', [preset.name, preset.sensors]);
  const textStyle = (x: number, y: number, size: number, muted = false): Style => ({
    line_box: true,
    x,
    y,
    size,
    align: 'center',
    color: muted ? [144, 177, 201] : [219, 247, 242],
  });
  const layers: Layer[] = [];
  if (id === 'clock') {
    layers.push(
      {
        kind: 'clock',
        format: 'date',
        twenty_four_hour: false,
        show_seconds: false,
        style: textStyle(240, 145, 28, true),
      },
      {
        kind: 'clock',
        format: 'time_without_period',
        twenty_four_hour: false,
        show_seconds: false,
        style: textStyle(240, 205, 64),
      },
      {
        kind: 'clock',
        format: 'period',
        twenty_four_hour: false,
        show_seconds: false,
        style: textStyle(240, 295, 28, true),
      },
    );
  } else if (id === 'cycle') {
    layers.push({
      kind: 'sensor_cycle',
      interval_seconds: 10,
      style: textStyle(240, 205, 32),
      sensors: bindings.map((sensor) => ({
        sensor_id: sensor.id,
        label: Array.from(sensor.label).slice(0, 26).join(''),
        unit: sensor.unit,
        temperature_unit: 'system',
        decimals: sensor.unit === 'rpm' ? 0 : sensor.unit === 'volts' ? 2 : 1,
      })),
    });
    layers.push({
      kind: 'clock',
      twenty_four_hour: true,
      show_seconds: false,
      style: textStyle(240, 365, 24, true),
    });
  } else {
    bindings.forEach((sensor, index) => {
      const x = id === 'quad' ? (index % 2 === 0 ? 150 : 330) : 240;
      const y =
        id === 'focus'
          ? 205
          : id === 'duo'
            ? 155 + index * 115
            : id === 'triple'
              ? 125 + index * 85
              : 175 + Math.floor(index / 2) * 120;
      layers.push({
        kind: 'text',
        text: Array.from(sensor.label)
          .slice(0, id === 'quad' ? 16 : 26)
          .join(''),
        style: textStyle(x, y - 36, 18, true),
      });
      layers.push({
        kind: 'sensor',
        sensor_id: sensor.id,
        label: '',
        unit: sensor.unit,
        temperature_unit: 'system',
        decimals: sensor.unit === 'rpm' ? 0 : sensor.unit === 'volts' ? 2 : 1,
        style: textStyle(x, y, id === 'focus' ? 64 : id === 'quad' ? 32 : 44),
      });
    });
    layers.push({
      kind: 'clock',
      twenty_four_hour: true,
      show_seconds: false,
      style: textStyle(240, 365, 24, true),
    });
  }
  return { version: 1, name: preset.name, background: [12, 18, 30], layers };
}
export function formatSensor(
  sensor: Sensor,
  temperatureUnit: 'celsius' | 'fahrenheit' = 'celsius',
): string {
  if (sensor.status !== 'valid' || sensor.value === null || !Number.isFinite(sensor.value))
    return '—';
  if (sensor.unit === 'celsius')
    return `${(temperatureUnit === 'fahrenheit' ? (sensor.value * 9) / 5 + 32 : sensor.value).toFixed(1)} ${temperatureUnit === 'fahrenheit' ? '°F' : '°C'}`;
  if (sensor.unit === 'bytes') return `${(sensor.value / 1024 ** 3).toFixed(1)} GiB`;
  if (sensor.unit === 'hertz') return `${(sensor.value / 1_000_000).toFixed(1)} MHz`;
  return `${sensor.value.toFixed(sensor.unit === 'rpm' ? 0 : sensor.unit === 'volts' ? 2 : 1)}${{ percent: '%', rpm: ' RPM', watts: ' W', volts: ' V', bytes_per_second: ' B/s', frames_per_second: ' FPS' }[sensor.unit]}`;
}
