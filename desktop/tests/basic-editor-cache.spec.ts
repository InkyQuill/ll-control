import { test, expect } from '@playwright/test';
import type { BasicOverlay, Theme } from '../src/lib/api';
const overlays:BasicOverlay[]=['none','slider_sensor','duo_sensor','three_sensor','four_sensor','time','custom'];
test('inactive basic resources and edits survive Basic → Rain → Basic without activating cached video',async({page})=>{
  await page.goto('/');
  const source:Theme={version:1,name:'Owned basic',background:[9,8,7],layers:[],basic_editor:{selected_background:'image_video',banks:[{background:'image_video',selected_overlay:'custom',overlays:overlays.map(overlay=>({overlay,theme:{version:2,name:overlay,background:[1,2,3],background_video:'a'.repeat(32),layers:[]}}))}]}};
  const result=await page.evaluate(async({modulePath,source})=>{
    const {switchLcdPage,themePages}=await import(/* @vite-ignore */modulePath);
    const before=JSON.stringify(source);
    const rain:Theme=switchLcdPage(source,'rain');
    const keys=themePages(rain).flatMap((theme:Theme)=>theme.background_video?[theme.background_video]:[]);
    const restored:Theme=switchLcdPage(rain,'basic');
    return {unchanged:before===JSON.stringify(source),rainHasBasic:!!rain.basic_editor,rainVideo:rain.background_video??null,keys,restored};
  },{modulePath:'/src/lib/lcd-pages.ts',source});
  expect(result.unchanged).toBe(true);expect(result.rainHasBasic).toBe(false);expect(result.rainVideo).toBeNull();
  expect(result.keys).toContain('a'.repeat(32));
  expect(result.restored.background).toEqual(source.background);expect(result.restored.layers).toEqual([]);expect(result.restored.background_video??null).toBeNull();
  expect(result.restored.basic_editor).toEqual(source.basic_editor);
});
