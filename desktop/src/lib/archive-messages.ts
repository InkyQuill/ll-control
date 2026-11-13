/** App-owned notices are translated by code; arbitrary diagnostics remain data. */
import { translate, type InterfaceLanguage } from './localization';

export interface ArchiveMessage {
  message: string | null;
  message_code?: string | null;
}
const resources = {
  windows_preparation_failed:
    'Windows-архив не подготовлен. Проверьте файл, record и привязки датчиков; повторите подготовку.',
  windows_import_unconfirmed:
    'Результат Windows импорта не подтверждён. Прочитайте результат по ID; новый импорт автоматически не запускался.',
  cancelled: 'Операция отменена.',
  pending_profile_operation:
    'Сначала прочитайте результат сохранённой операции или закройте её сведения после обновления списка.',
  pending_group_operation:
    'Сначала прочитайте результат сохранённой операции и закройте её сведения.',
  profile_export_unconfirmed:
    'Архив подготовлен в daemon; локальный файл не подтверждён. Отмените передачу и повторите экспорт.',
  group_export_unconfirmed:
    'Локальное сохранение файла не подтверждено. Отмените передачу и повторите экспорт.',
  cancel_unconfirmed: 'Отмена передачи не подтверждена; прочитайте результат по ID операции.',
  invalid_archive_file: 'Выберите обычный файл архива допустимого размера',
  invalid_profile_archive: 'Это не архив профиля LL-Control',
  invalid_manifest_length: 'Неверная длина manifest',
  destination_exists: 'Файл уже существует: выберите новое имя',
} as const;
/** Only explicit application call sites can assign a translatable notice code. */
export class ArchiveUiError extends Error {
  readonly message_code: keyof typeof resources;
  constructor(code: keyof typeof resources) {
    super(translate(resources[code], 'en-US'));
    this.message_code = code;
  }
}
function resource(value: ArchiveMessage): string | null {
  return typeof value.message_code === 'string' && Object.hasOwn(resources, value.message_code)
    ? resources[value.message_code as keyof typeof resources]
    : null;
}
export function archiveError(error: unknown): ArchiveMessage {
  const object = typeof error === 'object' && error !== null ? error : null;
  return {
    message: object && 'message' in object ? String(object.message) : String(error),
    message_code:
      object && 'message_code' in object && typeof object.message_code === 'string'
        ? object.message_code
        : null,
  };
}
export function archiveMessage(
  value: string | ArchiveMessage,
  language: InterfaceLanguage,
): string {
  if (typeof value === 'string') return translate(value, language);
  const source = resource(value);
  return source ? translate(source, language) : (value.message ?? '');
}
export function archiveDiagnostic(
  value: string | ArchiveMessage,
  language: InterfaceLanguage,
): string {
  return typeof value !== 'string' &&
    resource(value) &&
    value.message &&
    value.message !== archiveMessage(value, language)
    ? value.message
    : '';
}
