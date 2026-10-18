import {invoke} from '@tauri-apps/api/core';
export interface SystemFont {id:string;name:string}
let catalogue:Promise<SystemFont[]>|undefined;
export function systemFonts():Promise<SystemFont[]> {
  return catalogue??=invoke<SystemFont[]>('system_fonts',{}).then(fonts=>Array.isArray(fonts)?fonts:[]).catch(error=>{catalogue=undefined;throw error;});
}
export const importedSystemFonts=new Map<string,string>();
export async function systemFontFile(font:SystemFont):Promise<File> {
  const bytes=await invoke<ArrayBuffer>('system_font_read',{id:font.id});
  return new File([bytes],font.name+'.ttf');
}
