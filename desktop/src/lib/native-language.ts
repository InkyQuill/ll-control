import {invoke} from '@tauri-apps/api/core';
import type {InterfaceLanguage} from './localization';

let pending:InterfaceLanguage|undefined, applied:InterfaceLanguage|undefined;
let running=false;
/** The initial value is the last confirmed cache; later values are confirmed snapshots. */
export function syncNativeLanguage(language:InterfaceLanguage):void {
  pending=language;
  if(!running)void flush();
}
async function flush():Promise<void> {
  running=true;
  try {
    while(pending) {
      const language=pending;pending=undefined;
      if(language===applied)continue;
      try {await invoke('desktop_set_language',{language});applied=language;}
      catch(error) {applied=undefined;console.warn('Could not update native menu language',error);}
    }
  } finally {running=false;}
}
