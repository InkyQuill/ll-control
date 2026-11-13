import type { DisplaySettings } from './api';
import type { InterfaceLanguage } from './localization';
const languageKey = 'lconnect.interface-language';
function cachedLanguage(): InterfaceLanguage {
  if (typeof window === 'undefined') return 'en-US';
  try {
    return window.localStorage.getItem(languageKey) === 'ru-RU' ? 'ru-RU' : 'en-US';
  } catch {
    return 'en-US';
  }
}
export const presentation = $state<{
  settings: DisplaySettings | null;
  language: InterfaceLanguage;
}>({ settings: null, language: cachedLanguage() });
/** Only confirmed RPC snapshots update the cache; drafts cannot change the UI language. */
export function publishDisplaySettings(settings: DisplaySettings): void {
  presentation.settings = settings;
  presentation.language = settings.interface_language ?? 'ru-RU';
  if (typeof window !== 'undefined')
    try {
      window.localStorage.setItem(languageKey, presentation.language);
    } catch {
      /* The daemon record remains authoritative when local storage is unavailable. */
    }
}
