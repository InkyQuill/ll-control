import { test, expect } from '@playwright/test';
import type { ProfileLibrary, Theme } from '../src/lib/api';

declare global {
  interface Window {
    compositionFixture: {
      calls: { method: string; params: Record<string, unknown> }[];
      missing: boolean;
    };
  }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let active: unknown = null;
    const library: ProfileLibrary = { version: 1, revision: 0, profiles: [] };
    const fixture = { calls: [] as {method:string;params:Record<string,unknown>}[], missing:false };
    Object.assign(window, { compositionFixture: fixture, __TAURI_INTERNALS__: {
      invoke: async (command:string, req:{method:string;params:Record<string,unknown>}) => {
        if (command === 'desktop_set_language') return null;
        if (command === 'media_status' || command === 'media_thumbnail_cancel') return null;
        if (command === 'media_thumbnail') throw new Error('Thumbnail fixture');
        fixture.calls.push(structuredClone(req));
        switch (req.method) {
          case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices': return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
          case 'list_sensors': return [];
          case 'list_named_theme_assets': return [
            {key:'builtin-orbit-mark-v1',name:'Орбита',revision:0,is_default:true,contexts:['image','background','watermark']},
            {key:'builtin-midnight-v1',name:'Полночь',revision:0,is_default:true,contexts:['image','background']},
          ];
          case 'thumbnail_theme_asset': return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0));
          case 'get_lcd_state': return {source:active ? 'Theme':'Idle'};
          case 'get_lcd_media': return null;
          case 'list_prepared_clips': return fixture.missing ? [] : [{key:'a'.repeat(32),name:'Cinema',fps:24,frames:586,encoded_bytes:100}];
          case 'get_lcd_theme': return structuredClone(active);
          case 'set_lcd_theme':
            if (fixture.missing && (req.params.theme as Theme).background_video) throw new Error('Video asset missing');
            active = structuredClone(req.params.theme); return {source:'Theme'};
          case 'preview_lcd_theme':
            if (fixture.missing) throw new Error('Video asset missing');
            return [255,216,255,217];
          case 'list_profiles': return structuredClone(library);
          case 'edit_profile': {
            const edit = req.params.edit as {profile:ProfileLibrary['profiles'][number]};
            library.profiles = [structuredClone(edit.profile)]; library.revision++;
            return structuredClone(library);
          }
          default: throw new Error(req.method);
        }
      }
    }});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
});

test('video background preserves layers and gradient, previews and saves complete slot without implicit Apply', async ({ page }) => {
  const mode = page.getByRole('combobox',{name:'Тип фона',exact:true,includeHidden:true});
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await mode.selectOption('horizontal');
  await page.getByLabel('Первый цвет фона',{exact:true}).fill('#123456');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await page.getByRole('button',{name:'Использовать фоном темы',exact:true}).click();
  await expect(mode).toHaveValue('video');
  const writes = () => page.evaluate(() => window.compositionFixture.calls.filter(c=>c.method.startsWith('set_lcd')));
  expect(await writes()).toEqual([]);
  await page.getByRole('button',{name:'Обновить preview',exact:true}).click();
  await expect.poll(() => page.evaluate(() => window.compositionFixture.calls.filter(c=>c.method==='preview_lcd_theme').length)).toBe(1);
  expect(await writes()).toEqual([]);
  await page.getByText('Сохранённые профили',{exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await page.getByLabel('Имя профиля').fill('Video theme');
  await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await mode.selectOption('horizontal');
  await expect(page.getByLabel('Первый цвет фона',{exact:true})).toHaveValue('#123456');
  await page.getByRole('button',{name:'Загрузить профиль в редактор',exact:true}).click();
  await expect(mode).toHaveValue('video');
  expect(await writes()).toEqual([]);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(async()=> (await writes()).length).toBe(1);
  const composed = (await writes())[0].params.theme as Theme;
  expect(composed).toMatchObject({version:2,background_video:'a'.repeat(32)});
  expect(composed.layers.length).toBeGreaterThan(0);
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await mode.selectOption('solid');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(async()=> (await writes()).length).toBe(2);
  const solid = (await writes())[1].params.theme as Theme;
  expect(solid.version).toBe(1);
  expect(solid.background_video).toBeUndefined();
  expect(solid.layers).toEqual(composed.layers);
});

test('missing draft video is not silently replaced and failed Apply is visible', async ({ page }) => {
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await page.getByRole('button',{name:'Использовать фоном темы',exact:true}).click();
  await page.evaluate(() => { window.compositionFixture.missing = true; });
  await page.getByRole('button',{name:'Обновить ролики',exact:true}).click();
  await expect(page.getByRole('combobox',{name:'Тип фона',exact:true,includeHidden:true})).toHaveValue('video');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect(page.getByRole('alert').filter({hasText:'Video asset missing'})).toBeVisible();
  await expect(page.getByText('Источник: Нет программного потока',{exact:true})).toBeVisible();
});

test('image background and watermark contexts preserve overlays and switch exclusively with video',async({page})=>{
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  await page.getByLabel('Назначение изображения').selectOption('background');
  await page.getByRole('button',{name:'Сделать фоном Полночь',exact:true}).click();
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await expect(page.getByLabel('Тип фона')).toHaveValue('image');
  await page.getByLabel('Размещение фона').selectOption('contain');
  await page.getByRole('button',{name:'Библиотека',exact:true}).click();
  await page.getByLabel('Назначение изображения').selectOption('watermark');
  await expect(page.getByRole('button',{name:'Добавить водяной знак Полночь',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Добавить водяной знак Орбита',exact:true}).click();
  expect(await page.evaluate(()=>window.compositionFixture.calls.some(c=>c.method==='set_lcd_theme'))).toBe(false);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.compositionFixture.calls.filter(c=>c.method==='set_lcd_theme').length)).toBe(1);
  const first=await page.evaluate(()=>window.compositionFixture.calls.find(c=>c.method==='set_lcd_theme')!.params.theme as Theme);
  expect(first.background_image).toEqual({asset:'builtin-midnight-v1',fit:'contain'});
  expect(first.background_video).toBeUndefined();
  expect(first.layers.at(-1)).toMatchObject({kind:'image',asset:'builtin-orbit-mark-v1',fit:'contain_native',watermark_position:{horizontal:50,vertical:50}});
  expect(first.layers.some(l=>l.kind==='clock')).toBe(true);
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await page.getByRole('button',{name:'Использовать фоном темы',exact:true}).click();
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.compositionFixture.calls.filter(c=>c.method==='set_lcd_theme').length)).toBe(2);
  const video=await page.evaluate(()=>window.compositionFixture.calls.filter(c=>c.method==='set_lcd_theme').at(-1)!.params.theme as Theme);
  expect(video.background_image).toBeUndefined();expect(video.background_video).toBe('a'.repeat(32));
  expect(video.layers).toEqual(first.layers);
  await page.getByLabel('Назначение изображения').selectOption('background');
  await page.getByRole('button',{name:'Сделать фоном Полночь',exact:true}).click();
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await page.getByLabel('Тип фона').selectOption('solid');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.compositionFixture.calls.filter(c=>c.method==='set_lcd_theme').length)).toBe(3);
  const solid=await page.evaluate(()=>window.compositionFixture.calls.filter(c=>c.method==='set_lcd_theme').at(-1)!.params.theme as Theme);
  expect(solid.background_image).toBeUndefined();expect(solid.background_video).toBeUndefined();expect(solid.layers).toEqual(first.layers);
});
