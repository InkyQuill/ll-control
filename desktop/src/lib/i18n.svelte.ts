/** Reactive UI resources; persisted names, LCD text and hardware labels bypass this. */
import { presentation } from './presentation.svelte';
import { translate } from './localization';
import { ValidationError, type UiDiagnostic } from './validation-error';
export function t(source: string | null | undefined, ...values: unknown[]): string {
  return translate(source, presentation.language, ...values);
}

/** Only locally constructed resources are translated; remote diagnostics stay literal. */
export function diagnosticText(value: UiDiagnostic): string {
  return value instanceof ValidationError ? t(value.resource, ...value.values) : value;
}
