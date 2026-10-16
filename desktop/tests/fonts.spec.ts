import { test, expect } from '@playwright/test';

for (const mode of ['success','success-moved','lost-finish','cancel','cancel-moved','unmount'] as const) {
  test(`font import ${mode}: explicit selection, bounded chunks and no implicit apply`, async ({ page }) => {
    await page.addInitScript(({ mode }) => {
      const key = '0123456789abcdef0123456789abcdef';
      const calls: {method:string;params:Record<string,any>}[] = [];
      let received = 0, size = 0, asset: string|null = null, theme: unknown = null;
      Object.assign(window,{fontCalls:calls,__TAURI_INTERNALS__:{invoke: async (_:string, request:{method:string;params:Record<string,any>}) => {
        calls.push(structuredClone(request));
        const state = () => ({id:key,size,received,asset,cancelled:false});
        switch(request.method) {
          case 'list_devices':return [{id:'lcd:test',display_name:'LCDV2'}];
          case 'list_sensors':return [];
          case 'get_lcd_state':return {source:'Theme',last_error:null,requested:{mode:'Application',brightness_percent:100,rotation_degrees:0}};
          case 'set_lcd_theme': theme=structuredClone(request.params.theme);return {source:'Theme',last_error:null};
          case 'get_lcd_theme':return theme;
          case 'preview_lcd_theme':return [255,216,255,217];
          case 'font_upload': {
            const r=request.params.request;
            if(r.action==='begin')size=r.size;
            if(r.action==='chunk') { if(mode==='success-moved'||mode==='cancel'||mode==='cancel-moved'||mode==='unmount')await new Promise(r=>setTimeout(r,500)); received+=r.bytes.length; }
            if(r.action==='finish') { asset=key; if(mode==='lost-finish')throw new Error('lost response'); }
            return state();
          }
          default: throw new Error(request.method);
        }
      }}});
    },{mode});
    await page.goto('/');
    await expect(page.getByText('Daemon подключён')).toBeVisible();
    await page.getByText('Пользовательские шрифты',{exact:true}).click();
    await page.getByLabel('Загрузить шрифт',{exact:true}).focus();
    await page.getByLabel('Загрузить шрифт',{exact:true}).setInputFiles({name:'Custom.ttf',mimeType:'font/ttf',buffer:Buffer.alloc(70000,1)});
    if(mode==='success-moved') await page.getByLabel('Название',{exact:true}).focus();
    if(mode==='unmount') {
      await page.getByRole('button',{name:'Настройки',exact:true}).click();
      await expect.poll(async()=>page.evaluate(()=> (window as any).fontCalls.filter((c:any)=>c.method==='font_upload'&&c.params.request.action==='cancel').length)).toBe(1);
      const calls=await page.evaluate(()=> (window as any).fontCalls);
      expect(calls.some((c:any)=>c.method==='set_lcd_theme'||(c.method==='font_upload'&&c.params.request.action==='finish'))).toBe(false);
      return;
    }
    if(mode==='cancel'||mode==='cancel-moved') {
      const cancel = page.getByRole('button',{name:'Отменить загрузку шрифта',exact:true});
      await cancel.focus();
      await page.keyboard.press('Enter');
      if(mode==='cancel-moved') await page.getByLabel('Название',{exact:true}).focus();
      await expect(page.getByText('Загрузка отменена. Тема не изменена.')).toBeVisible();
      await expect(page.getByLabel(mode==='cancel-moved' ? 'Название' : 'Загрузить шрифт',{exact:true})).toBeFocused();
      await expect(page.getByRole('combobox',{name:'Шрифт',exact:true}).first()).toHaveValue('sans');
      const calls = await page.evaluate(()=> (window as any).fontCalls);
      expect(calls.some((c:any)=>c.method==='font_upload'&&c.params.request.action==='finish')).toBe(false);
    } else {
      await expect(page.getByText('Шрифт загружен.',{exact:false})).toBeVisible();
      await expect(page.getByLabel(mode==='success-moved' ? 'Название' : 'Загрузить шрифт',{exact:true})).toBeFocused();
      const select=page.getByRole('combobox',{name:'Шрифт',exact:true}).first();
      await expect(select).toHaveValue('sans');
      await select.selectOption('asset:0123456789abcdef0123456789abcdef');
      let calls=await page.evaluate(()=> (window as any).fontCalls);
      expect(calls.filter((c:any)=>c.method==='set_lcd_theme')).toHaveLength(0);
      const chunks=calls.filter((c:any)=>c.method==='font_upload'&&c.params.request.action==='chunk');
      expect(chunks.map((c:any)=>[c.params.request.offset,c.params.request.bytes.length])).toEqual([[0,65536],[65536,4464]]);
      await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
      await expect.poll(async()=>page.evaluate(()=> (window as any).fontCalls.filter((c:any)=>c.method==='set_lcd_theme').length)).toBe(1);
      calls=await page.evaluate(()=> (window as any).fontCalls);
      expect(calls.find((c:any)=>c.method==='set_lcd_theme').params.theme.layers[0].style.font_asset).toBe('0123456789abcdef0123456789abcdef');
      await select.selectOption('serif');
      await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
      await expect.poll(async()=>page.evaluate(()=> (window as any).fontCalls.filter((c:any)=>c.method==='set_lcd_theme').length)).toBe(2);
      calls=await page.evaluate(()=> (window as any).fontCalls);
      expect(calls.filter((c:any)=>c.method==='set_lcd_theme')[1].params.theme.layers[0].style.font_asset).toBeUndefined();
      if(mode==='lost-finish')expect(calls.filter((c:any)=>c.method==='font_upload'&&c.params.request.action==='status')).toHaveLength(1);
    }
    const calls=await page.evaluate(()=> (window as any).fontCalls);
    expect(calls.filter((c:any)=>c.method==='font_upload'&&c.params.request.action==='cancel')).toHaveLength(1);
  });
}
