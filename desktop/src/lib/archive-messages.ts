/** App-owned notices are translated by code; arbitrary diagnostics remain data. */
import { translate, type InterfaceLanguage } from './localization';

export interface ArchiveMessage {
  message: string | null;
  message_code?: string | null;
}
const resources = {
  windows_preparation_failed:
    'Windows archive is not prepared. Check the file, controller record and sensor bindings, then prepare again.',
  windows_import_unconfirmed:
    'The Windows import result is unconfirmed. Read the operation ID; no new import was started automatically.',
  cancelled: 'Operation cancelled.',
  pending_profile_operation:
    'Read the saved operation result or refresh the list and close its details first.',
  pending_group_operation: 'Read the saved operation result and close its details first.',
  profile_export_unconfirmed:
    'The daemon prepared the archive, but the local file is unconfirmed. Cancel the transfer and export again.',
  group_export_unconfirmed: 'The local file is unconfirmed. Cancel the transfer and export again.',
  cancel_unconfirmed:
    'Transfer cancellation is unconfirmed. Read the result using the operation ID.',
  invalid_archive_file: 'Choose a regular archive file within the size limit.',
  invalid_profile_archive: 'This is not an LL-Control profile archive.',
  invalid_manifest_length: 'Invalid archive manifest length.',
  destination_exists: 'The file already exists. Choose a new name.',
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
