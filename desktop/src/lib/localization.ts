/** Explicit UI resources. Parameters are passed separately and never inferred from names. */
import en from './locales/en-US.json' with { type: 'json' };
export type InterfaceLanguage = 'ru-RU' | 'en-US';
const english: Readonly<Record<string, string>> = en;
const format = (resource: string, values: unknown[]) =>
  resource.replace(/\{(\d+)\}/g, (_, index: string) => String(values[Number(index)] ?? ''));
export function translate(
  source: string | null | undefined,
  language: InterfaceLanguage,
  ...values: unknown[]
): string {
  if (source === null || source === undefined) return '';
  if (language === 'ru-RU') return values.length ? format(source, values) : source;
  const key = source.trim().replace(/\s+/g, ' ');
  const resource = Object.hasOwn(english, key) ? english[key] : undefined;
  if (resource === undefined) return values.length ? format(source, values) : source;
  const leading = source.match(/^\s*/)?.[0] ?? '',
    trailing = source.match(/\s*$/)?.[0] ?? '';
  return leading + format(resource, values) + trailing;
}
