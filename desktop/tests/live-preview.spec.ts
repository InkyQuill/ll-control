import {test,expect} from '@playwright/test';
test('live preview coalesces edits, discards obsolete responses and stops without device writes',async({page})=>{
  await page.addInitScript(()=>{
    let active=0,max=0;
    const requests: {method:string;params:Record<string,unknown>}[]=[];
    const pending: (()=>void)[]=[];
    Object.assign(window,{previewTest:{requests,pending,get max(){return max;}},__TAURI_INTERNALS__:{invoke:async(_cmd:string,req:{method:string;params:Record<string,unknown>})=>{
        if (_cmd === 'desktop_set_language') return null;
      requests.push(req);
      switch(req.method){
        case 'list_devices':return [{id:'lcd:test',display_name:'LCD', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle',last_error:null,requested:{}};
        case 'preview_lcd_editor':
          active++;max=Math.max(max,active);
          await new Promise<void>(resolve=>pending.push(resolve));active--;
          return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII='),c=>c.charCodeAt(0));
        default:throw new Error('Unexpected '+req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  const count=()=>page.evaluate(()=>(window as unknown as {previewTest:{requests:{method:string}[]}}).previewTest.requests.filter(r=>r.method==='preview_lcd_editor').length);
  const release=()=>page.evaluate(()=>(window as unknown as {previewTest:{pending:(()=>void)[]}}).previewTest.pending.shift()?.());
  await expect.poll(count).toBe(1);
  await page.getByLabel('Название',{exact:true}).fill('Новая тема');
  await release();
  await expect(page.getByRole('img')).toHaveCount(0);
  await expect.poll(count).toBe(2);
  const latest=await page.evaluate(()=>(window as unknown as {previewTest:{requests:{method:string;params:{theme?:{name:string}}}[]}}).previewTest.requests.filter(r=>r.method==='preview_lcd_editor').at(-1));
  expect(latest?.params.theme?.name).toBe('Новая тема');
  await release();
  await expect(page.getByRole('img')).toBeVisible();
  await expect.poll(count).toBe(3);
  await page.getByRole('button',{name:'Приостановить анимацию',exact:true}).click();
  await release();
  // Pause requests one authoritative still frame, then stops the playback loop.
  await expect.poll(count).toBe(4);await release();
  await expect(page.getByRole('img')).toBeVisible();
  await page.waitForTimeout(1300);expect(await count()).toBe(4);
  await page.getByLabel('Название',{exact:true}).fill('Paused edit');
  await expect.poll(count).toBe(5);await release();
  await page.waitForTimeout(1300);expect(await count()).toBe(5);
  await page.getByRole('button',{name:'Продолжить анимацию',exact:true}).click();
  await expect.poll(count).toBe(6);
  await page.getByRole('button',{name:'Датчики',exact:true}).click();await release();
  await page.waitForTimeout(1300);expect(await count()).toBe(6);
  await page.getByRole('button',{name:'Экран помпы',exact:true}).click();
  await expect.poll(count).toBe(7);await release();
  await expect(page.getByRole('img')).toBeVisible();
  const evidence=await page.evaluate(()=>(window as unknown as {previewTest:{max:number;requests:{method:string}[]}}).previewTest);
  expect(evidence.max).toBe(1);
  expect(evidence.requests.some(r=>r.method==='set_lcd_theme'||r.method==='clear_lcd')).toBe(false);
});
