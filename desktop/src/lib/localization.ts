/** Explicit UI resources. Parameters are passed separately and never inferred from names. */
import ru from './locales/ru-RU.json' with { type: 'json' };
export type InterfaceLanguage = 'ru-RU' | 'en-US';
const russian: Readonly<Record<string, string>> = ru;
const format = (resource: string, values: unknown[]) =>
  resource.replace(/\{(\d+)\}/g, (_, index: string) => String(values[Number(index)] ?? ''));
export function translate(
  source: string | null | undefined,
  language: InterfaceLanguage,
  ...values: unknown[]
): string {
  if (source === null || source === undefined) return '';
  if (language === 'en-US') return values.length ? format(source, values) : source;
  const key = source.trim().replace(/\s+/g, ' ');
  const resource = Object.hasOwn(russian, key) ? russian[key] : undefined;
  if (resource === undefined) return values.length ? format(source, values) : source;
  const leading = source.match(/^\s*/)?.[0] ?? '',
    trailing = source.match(/\s*$/)?.[0] ?? '';
  return leading + format(resource, values) + trailing;
}
