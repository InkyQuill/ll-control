export type DisplayUnit = 'celsius' | 'fahrenheit';
export function temperatureFromCelsius(
  value: number,
  unit: DisplayUnit,
  difference = false,
): number {
  return unit === 'fahrenheit' ? (value * 9) / 5 + (difference ? 0 : 32) : value;
}
export function temperatureToCelsius(
  value: number | undefined,
  unit: DisplayUnit,
  difference = false,
): number {
  if (value === undefined || !Number.isFinite(value)) return NaN;
  if (unit === 'celsius') return value;
  return Number((((value - (difference ? 0 : 32)) * 5) / 9).toFixed(8));
}
export function temperatureSuffix(unit: DisplayUnit): string {
  return unit === 'fahrenheit' ? '°F' : '°C';
}
