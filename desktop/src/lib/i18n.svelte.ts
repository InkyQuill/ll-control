/** Reactive UI resources; persisted names, LCD text and hardware labels bypass this. */
import {presentation} from './presentation.svelte';
import {translate} from './localization';
export function t(source:string|null|undefined,...values:unknown[]):string {
 return translate(source,presentation.language,...values);
}
