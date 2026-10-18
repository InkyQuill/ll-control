import { test, expect, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

let videoFixture:Buffer, splitFixture:Buffer, longFixture:Buffer, videoDirectory:string;
test.beforeAll(()=>{
  videoDirectory=mkdtempSync(join(tmpdir(),'lcd-video-preview-'));
  const file=join(videoDirectory,'preview.mp4');
  execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','color=red:s=64x64:r=24:d=1[r];color=blue:s=64x64:r=24:d=1[b];[r][b]concat=n=2:v=1:a=0','-c:v','libx264','-pix_fmt','yuv420p','-threads','1',file]);
  videoFixture=readFileSync(file);
  const split=join(videoDirectory,'split.mp4');
  execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','color=red:s=128x64:r=24:d=2,drawbox=x=64:y=0:w=64:h=64:color=blue:t=fill','-c:v','libx264','-pix_fmt','yuv420p','-threads','1',split]);
  splitFixture=readFileSync(split);
  const long=join(videoDirectory,'long.mp4');
  execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','color=teal:s=16x16:r=1:d=200','-c:v','libx264','-pix_fmt','yuv420p','-threads','1','-movflags','+faststart',long]);
  longFixture=readFileSync(long);
});
test.afterAll(()=>{if(videoDirectory)rmSync(videoDirectory,{recursive:true,force:true});});

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    type Metadata={width:number;height:number;durationSeconds:number;averageFrameRate:{numerator:number;denominator:number}|null;nominalFrameRate:{numerator:number;denominator:number}|null};
    const metadata:Metadata={width:64,height:64,durationSeconds:2,averageFrameRate:{numerator:24,denominator:1},nominalFrameRate:{numerator:24,denominator:1}};
    type Job = {id:string;name:string;total:number;received:number;phase:string;key:string|null;message:string|null;metadata?:Metadata};
    type Collections = {version:number;revision:number;images:string[];watermarks:string[];backgrounds:{kind:string;key:string}[]};
    const fixture = {language:'ru-RU',cancelFailure:false,delayCancel:false,cancelRelease:null as null|(()=>void),statusReads:0,failStatus:false,diagnostic:'Настройки',failCommand:'',delayPostEditCatalog:false,completedCatalogReads:0,catalogRelease:null as null|(()=>void),metadata,delayInspect:false,lostInspect:false,inspectRelease:null as null|(()=>void),source:null as null|{id:string;key:string;phase:string;message:string|null;metadata?:Metadata},clips:[] as {key:string;name:string;fps:number;frames:number;encoded_bytes:number;is_default?:boolean}[],collections:null as Collections|null,membershipDevices:[] as string[],job:null as Job|null, calls:[] as {command:string;offset?:number;length?:number;fit?:string;cut?:unknown;adjustments?:unknown;range?:unknown}[], complete:false, reviewReady:false, delayConfirm:false, lostConfirm:false, confirmRelease:null as null|(()=>void), delayBegin:false, release:null as null|(()=>void)};
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{convertFileSrc:(id:string,protocol='media-preview')=>`http://${protocol}.localhost/${id}`,invoke:async(command:string,args:Record<string,unknown> = {})=>{
        if (command === 'desktop_set_language') return null;
      if(command===fixture.failCommand){fixture.failCommand='';fixture.calls.push({command});throw new Error(fixture.diagnostic);}
      if(command==='media_playback_url')return `http://${args.kind==='source' ? 'media-source' : 'media-preview'}.localhost/${args.id}`;
      if(command==='media_source_start') {fixture.calls.push({command});fixture.source={id:args.id as string,key:args.key as string,phase:'ready',message:null,metadata:fixture.metadata};return {...fixture.source};}
      if(command==='media_source_status')return fixture.source ? {...fixture.source}:null;
      if(command==='media_source_cancel') {fixture.calls.push({command});if(fixture.source && fixture.source.id===args.id)fixture.source.phase='cancelled';return fixture.source;}
      if(command==='media_source_prepare') {fixture.calls.push({command,fit:args.fit as string,cut:args.cut,...(args.range ? {range:args.range}:{}),...(args.adjustments ? {adjustments:args.adjustments}: {})});fixture.job={id:args.id as string,name:args.name as string,total:100,received:100,phase:'review_ready',key:null,message:null};return null;}
      if(command === 'media_thumbnail_cancel') return null;
      if(command === 'media_thumbnail') return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='), c=>c.charCodeAt(0));
      if(command==='media_status') {
        fixture.statusReads++;
        if(fixture.failStatus)throw new Error(fixture.diagnostic);
        if(fixture.complete && fixture.job?.phase==='encoding') {fixture.job.phase='complete';fixture.job.key='a'.repeat(32);}
        return fixture.job ? {...fixture.job}:null;
      }
      if(command==='media_begin') {
        fixture.calls.push({command,fit:args.fit as string,...(args.cut ? {cut:args.cut}: {}),...(args.adjustments ? {adjustments:args.adjustments}: {})});
        if(fixture.delayBegin) await new Promise<void>(resolve=>{fixture.release=resolve;});
        fixture.job={id:args.id as string,name:args.name as string,total:args.total as number,received:0,phase:'uploading',key:null,message:null};
        return {...fixture.job};
      }
      if(command==='media_append') {
        const bytes=args.bytes as number[];
        fixture.calls.push({command,offset:args.offset as number,length:bytes.length});
        if(!fixture.job || fixture.job.phase!=='uploading') throw new Error('Not uploading');
        fixture.job.received+=bytes.length; return {...fixture.job};
      }
      if(command==='media_inspect') {
        fixture.calls.push({command});
        if(!fixture.job || fixture.job.received!==fixture.job.total)throw new Error('Incomplete upload');
        fixture.job.phase='inspecting';
        if(fixture.delayInspect)await new Promise<void>(resolve=>fixture.inspectRelease=resolve);
        if(fixture.job.phase!=='cancelled') {fixture.job.phase='source_ready';fixture.job.metadata=fixture.metadata;}
        if(fixture.lostInspect)throw new Error('Lost inspection reply');
        return {...fixture.job};
      }
      if(command==='media_prepare_inspected') {
        fixture.calls.push({command,fit:args.fit as string,range:args.range,...(args.adjustments ? {adjustments:args.adjustments}:{})});
        if(!fixture.job || fixture.job.phase!=='source_ready')throw new Error('Not inspected');
        fixture.job.phase='review_ready';return null;
      }
      if(command==='media_start') {fixture.calls.push({command});if(fixture.job)fixture.job.phase=args.review && fixture.reviewReady ? 'review_ready' : 'encoding';return null;}
      if(command==='media_confirm') {
        fixture.calls.push({command});
        if(!fixture.job || fixture.job.phase!=='review_ready')throw new Error('Not awaiting review');
        fixture.job.phase='transferring';
        if(fixture.delayConfirm)await new Promise<void>(resolve=>fixture.confirmRelease=resolve);
        if(fixture.job.phase!=='cancelled') {fixture.job.phase='complete';fixture.job.key='a'.repeat(32);}
        if(fixture.lostConfirm)throw new Error('Lost confirmation reply');
        return null;
      }
      if(command==='media_cancel') {fixture.calls.push({command});if(fixture.delayCancel)await new Promise<void>(resolve=>fixture.cancelRelease=resolve);if(fixture.cancelFailure){fixture.cancelFailure=false;throw new Error(fixture.diagnostic);}if(fixture.job)fixture.job.phase='cancelled';return fixture.job?{...fixture.job}:null;}
      if(command==='daemon_call') {
        switch(args.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:fixture.language};
          case 'get_screen_collections': return {collections:fixture.collections,persistence:{pending:false,saved:false,last_error:null}};
        case 'set_screen_collections': {const params=args.params as {dev:string;expected_revision:number;collections:Collections};if(fixture.collections && params.expected_revision!==fixture.collections.revision)throw new Error('Collection revision conflict');fixture.membershipDevices.push(params.dev);fixture.collections=params.collections;return fixture.collections;}
        case 'list_devices': return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'},{id:'B',display_name:'LCD B', family:'galahad2-lcdv2'}];
          case 'list_sensors': return [];
          case 'get_lcd_state': return {source:'Idle'};
          case 'get_lcd_media': return null;
          case 'list_prepared_clips':
            if(fixture.delayPostEditCatalog && fixture.job?.phase==='complete' && ++fixture.completedCatalogReads===2)await new Promise<void>(resolve=>fixture.catalogRelease=resolve);
            return [...fixture.clips,...(fixture.job?.phase==='complete'?[{key:'a'.repeat(32),name:fixture.job.name,fps:24,frames:24,encoded_bytes:100}]:[])];
          default:fixture.calls.push({command:args.method as string});throw new Error(String(args.method));
        }
      }
      throw new Error(command);
    }}});
  });
  await page.goto('/?view=display');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await expect(page.getByLabel('Импортировать GIF/MP4')).toBeEnabled();
});

test('uploads bounded chunks, reports conversion, refreshes library without applying',async({page})=>{
  await page.getByLabel('Размещение видео').selectOption('cover');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'clouds.mp4',mimeType:'video/mp4',buffer:Buffer.alloc(600_000)});
  await expect(page.getByText('Конвертация для LCD: clouds.mp4',{exact:true})).toBeVisible();
  await expect(page.getByLabel('Импортировать GIF/MP4')).toBeDisabled();
  await page.evaluate(()=>{(window as unknown as {fixture:{complete:boolean}}).fixture.complete=true;});
  await expect(page.getByText('Ролик сохранён в библиотеке: clouds.mp4',{exact:true})).toBeVisible();
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('a'.repeat(32));
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:unknown[]}}).fixture.calls);
  expect(calls).toEqual([{command:'media_begin',fit:'cover'},
    {command:'media_append',offset:0,length:262144},{command:'media_append',offset:262144,length:262144},
    {command:'media_append',offset:524288,length:75712},{command:'media_start'}]);
});

test('cancel conversion permits explicit retry and invalid input remains visible',async({page})=>{
  const input=page.getByLabel('Импортировать GIF/MP4');
  await input.setInputFiles({name:'bad.txt',mimeType:'text/plain',buffer:Buffer.from('bad')});
  await expect(page.getByRole('alert')).toContainText('Выберите непустой GIF');
  await input.setInputFiles({name:'x.gif',mimeType:'image/gif',buffer:Buffer.from('fixture')});
  await expect(page.getByText('Конвертация для LCD: x.gif',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Отменить импорт',exact:true}).click();
  await expect(page.getByText('Импорт отменён: x.gif',{exact:true})).toBeVisible();
  await expect(input).toBeEnabled();
});

test('closing panel during delayed begin cancels its eventual upload without starting',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{delayBegin:boolean}}).fixture.delayBegin=true;});
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'x.gif',mimeType:'image/gif',buffer:Buffer.from('fixture')});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{release:unknown}}).fixture.release!==null)).toBe(true);
  await page.keyboard.press('Escape');
  await page.evaluate(()=>{(window as unknown as {fixture:{release:()=>void}}).fixture.release();});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}|null}}).fixture.job?.phase)).toBe('cancelled');
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.map(c=>c.command));
  expect(calls).not.toContain('media_start');
  expect(calls).not.toContain('media_append');
});

test('transfer can be cancelled but publication waits for its outcome',async({page})=>{
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'long.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')});
  await page.evaluate(()=>{const f=(window as unknown as {fixture:{job:{phase:string}}}).fixture;f.job.phase='transferring';});
  await expect(page.getByText('Передача ролика в библиотеку: long.mp4',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Отменить импорт',exact:true})).toBeEnabled();
  await page.evaluate(()=>{const f=(window as unknown as {fixture:{job:{phase:string}}}).fixture;f.job.phase='publishing';});
  await expect(page.getByRole('button',{name:'Отменить импорт',exact:true})).toBeDisabled();
  await expect(page.getByText('Сохранение уже началось. Дождитесь результата перед повторным импортом.')).toBeVisible();
  await expect(page.getByLabel('Импортировать GIF/MP4')).toBeDisabled();
});

test('completed owned video import adds membership to its initiating device',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{collections:unknown}}).fixture.collections={version:1,revision:2,images:[],watermarks:[],backgrounds:[]};});
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'target.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')});
  await expect(page.getByText('Конвертация для LCD: target.mp4',{exact:true})).toBeVisible();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await page.evaluate(()=>{(window as unknown as {fixture:{complete:boolean}}).fixture.complete=true;});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{membershipDevices:string[]}}).fixture.membershipDevices)).toEqual(['A']);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{collections:{revision:number;backgrounds:unknown[]}}}).fixture.collections)).toMatchObject({revision:3,backgrounds:[{kind:'video',key:'a'.repeat(32)}]});
});


test('ordered video selection validates duration and sends millisecond intervals',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  const input=page.getByLabel('Импортировать GIF/MP4');
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('181');
  await expect(input).toBeEnabled();
  await page.getByLabel('Начало фрагмента 1, с',{exact:true}).fill('2.25');
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('2.75');
  await page.getByRole('button',{name:'Добавить фрагмент'}).click();
  await page.getByLabel('Начало фрагмента 2, с',{exact:true}).fill('0.25');
  await page.getByLabel('Конец фрагмента 2, с',{exact:true}).fill('0.75');
  await input.setInputFiles({name:'selected.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')});
  const submit=page.getByRole('button',{name:'Подготовить выбранные фрагменты'});
  await submit.click();
  await expect(page.getByLabel('Начало фрагмента 1, с',{exact:true})).toBeDisabled();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:unknown}[]}}).fixture.calls);
  expect(calls.find(c=>c.command==='media_begin')?.cut).toEqual({segments:[{start_ms:2250,end_ms:2750},{start_ms:250,end_ms:750}]});
  await page.getByRole('button',{name:'Отменить импорт'}).click();
  await expect(input).toBeEnabled();
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('');
  await expect(submit).toBeDisabled();
  await page.getByLabel('Выбрать фрагменты видео').uncheck();
  await expect(input).toBeEnabled();
});


test('removing the last video fragment preserves preceding values and keyboard focus',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Начало фрагмента 1, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Добавить фрагмент'}).click();
  const remove=page.getByRole('button',{name:'Удалить фрагмент 2',exact:true});
  await remove.focus();await page.keyboard.press('Enter');
  const first=page.getByLabel('Начало фрагмента 1, с',{exact:true});
  await expect(first).toBeFocused();await expect(first).toHaveValue('1');
  await expect(page.getByRole('button',{name:'Удалить фрагмент 1',exact:true})).toBeDisabled();
});


test('local source preview seeks real frames under app CSP and sets cut boundaries before upload',async({page})=>{
  const config=JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
  await page.route('http://127.0.0.1:1420/',async route=>{
    const response=await route.fetch();
    // Tauri injects hashes for bootstrap scripts. Mirror that for Vite's inline
    // bootstrap while retaining the actual configured media-src restriction.
    const html=await response.text();
    const hashes=Array.from(html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g))
      .filter(m=>!m[1].includes('src='))
      .map(m=>`'sha256-${createHash('sha256').update(m[2]).digest('base64')}'`);
    await route.fulfill({response,headers:{...response.headers(),'content-security-policy':`${config.app.security.csp}; script-src 'self' ${hashes.join(' ')}`}});
  });
  await page.reload();
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'preview.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const video=page.getByLabel('Исходное видео',{exact:true});
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  const submit=page.getByRole('button',{name:'Подготовить выбранные фрагменты'});
  await expect(submit).toBeDisabled(); // Default end5s exceeds this actual2s source.
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.some(c=>c.command==='media_begin'))).toBe(false);
  async function seek(seconds:number) {
    await page.getByLabel('Позиция в исходном видео').evaluate((input,time)=>{(input as HTMLInputElement).value=String(time);input.dispatchEvent(new Event('input',{bubbles:true}));},seconds);
    await expect(page.getByRole('button',{name:'Начало по текущему кадру 1',exact:true})).toBeEnabled();
    await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeCloseTo(seconds,2);
    return video.evaluate(v=>{const c=document.createElement('canvas');c.width=1;c.height=1;const x=c.getContext('2d')!;x.drawImage(v as HTMLVideoElement,0,0,1,1);return Array.from(x.getImageData(0,0,1,1).data);});
  }
  const red=await seek(.5);expect(red[0]).toBeGreaterThan(200);expect(red[2]).toBeLessThan(30);
  await page.getByRole('button',{name:'Начало по текущему кадру 1',exact:true}).click();
  const blue=await seek(1.5);expect(blue[2]).toBeGreaterThan(200);expect(blue[0]).toBeLessThan(30);
  await page.getByRole('button',{name:'Конец по текущему кадру 1',exact:true}).click();
  await expect(page.getByLabel('Начало фрагмента 1, с',{exact:true})).toHaveValue('0.5');
  await expect(page.getByLabel('Конец фрагмента 1, с',{exact:true})).toHaveValue('1.5');
  await submit.click();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:unknown}[]}}).fixture.calls);
  expect(calls.find(c=>c.command==='media_begin')?.cut).toEqual({segments:[{start_ms:500,end_ms:1500}]});
  await expect(video).not.toHaveAttribute('controls');
});

test('changing and closing local preview releases URLs and an unreadable source permits manual selection',async({page})=>{
  await page.evaluate(()=>{
    const revoked:string[]=[];Object.assign(window,{previewRevoked:revoked});
    const original=URL.revokeObjectURL.bind(URL);URL.revokeObjectURL=(url)=>{revoked.push(url);original(url);};
  });
  await page.getByLabel('Выбрать фрагменты видео').check();
  const input=page.getByLabel('Импортировать GIF/MP4');
  await input.setInputFiles({name:'first.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const video=page.getByLabel('Исходное видео',{exact:true});
  await expect(video).toHaveAttribute('src',/^blob:/);
  const first=await video.getAttribute('src');
  await input.setInputFiles({name:'unreadable.mp4',mimeType:'video/mp4',buffer:Buffer.from('invalid')});
  await expect(page.getByText('Просмотр этого файла недоступен.',{exact:false})).toBeVisible();
  await expect(page.getByRole('button',{name:'Начало по текущему кадру 1',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Подготовить выбранные фрагменты'})).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {previewRevoked:string[]}).previewRevoked)).toContain(first);
  const second=await video.getAttribute('src');
  await page.keyboard.press('Escape');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {previewRevoked:string[]}).previewRevoked)).toContain(second);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.some(c=>c.command==='media_begin'))).toBe(false);
});


test('marking a boundary reads the player even between timeupdate notifications',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'playback.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const video=page.getByLabel('Исходное видео',{exact:true});
  const mark=page.getByRole('button',{name:'Начало по текущему кадру 1',exact:true});
  await expect(mark).toBeEnabled();
  await video.evaluate(async v=>{v.addEventListener('timeupdate',e=>e.stopImmediatePropagation(),{capture:true});await (v as HTMLVideoElement).play();});
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeGreaterThan(.25);
  const position=await video.evaluate(v=>{(v as HTMLVideoElement).pause();return (v as HTMLVideoElement).currentTime;});
  await mark.click();
  const value=await page.getByLabel('Начало фрагмента 1, с',{exact:true}).inputValue();
  expect(Number(value)).toBeCloseTo(position,2);
});


test('per-fragment crop previews real selected pixels and sends normalized validated rectangles',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'split.mp4',mimeType:'video/mp4',buffer:splitFixture});
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true}).click();
  const preview=page.getByLabel('Кадр после обрезки',{exact:true});
  await expect(preview).toBeVisible();
  await page.getByLabel('Ширина, % — фрагмент 1',{exact:true}).fill('50');
  await page.getByLabel('Слева, % — фрагмент 1',{exact:true}).fill('50');
  const pixels=()=>preview.evaluate(c=>Array.from((c as HTMLCanvasElement).getContext('2d')!.getImageData(240,240,1,1).data));
  await expect.poll(async()=> (await pixels())[2]).toBeGreaterThan(200);
  expect((await pixels())[0]).toBeLessThan(30);
  const submit=page.getByRole('button',{name:'Подготовить выбранные фрагменты'});
  await page.getByLabel('Ширина, % — фрагмент 1',{exact:true}).fill('51');
  await expect(submit).toBeDisabled();await expect(preview).toHaveCount(0);
  await page.getByLabel('Ширина, % — фрагмент 1',{exact:true}).fill('');
  await expect(submit).toBeDisabled();
  await page.getByLabel('Ширина, % — фрагмент 1',{exact:true}).fill('50');
  await page.getByRole('button',{name:'Добавить фрагмент'}).click();
  await page.getByLabel('Конец фрагмента 2, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Выбрать область фрагмента 2',exact:true}).click();
  await page.getByLabel('Ширина, % — фрагмент 2',{exact:true}).fill('50');
  await expect.poll(async()=> (await pixels())[0]).toBeGreaterThan(200);
  expect((await pixels())[2]).toBeLessThan(30);
  // A short rectangle must show bars in contain mode and fill them in cover mode.
  await page.getByLabel('Высота, % — фрагмент 2',{exact:true}).fill('25');
  const top=()=>preview.evaluate(c=>Array.from((c as HTMLCanvasElement).getContext('2d')!.getImageData(240,20,1,1).data));
  await expect.poll(async()=> (await top())[0]).toBeLessThan(20);
  await page.getByLabel('Размещение видео').selectOption('cover');
  await expect.poll(async()=> (await top())[0]).toBeGreaterThan(200);
  await page.getByLabel('Высота, % — фрагмент 2',{exact:true}).fill('100');
  await submit.click();
  await expect(page.getByLabel('Слева, % — фрагмент 1',{exact:true})).toBeDisabled();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:unknown}[]}}).fixture.calls);
  expect(calls.find(c=>c.command==='media_begin')?.cut).toEqual({segments:[
    {start_ms:0,end_ms:1000,crop:{x:.5,y:0,width:.5,height:1}},
    {start_ms:0,end_ms:1000,crop:{x:0,y:0,width:.5,height:1}},
  ]});
});

test('crop pointer selection, keyboard movement, row removal and reset keep the selected fragment consistent',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'split.mp4',mimeType:'video/mp4',buffer:splitFixture});
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Добавить фрагмент'}).click();
  await page.getByLabel('Конец фрагмента 2, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Выбрать область фрагмента 2',exact:true}).click();
  const surface=page.getByRole('button',{name:'Выбор области видео',exact:true});
  await expect(surface).toBeVisible();await surface.scrollIntoViewIfNeeded();
  const box=(await surface.boundingBox())!;
  await page.mouse.move(box.x+box.width*.45,box.y+box.height*.9);await page.mouse.down();
  await page.mouse.move(box.x+box.width*.05,box.y+box.height*.1);await page.mouse.up();
  expect(Number(await page.getByLabel('Слева, % — фрагмент 2',{exact:true}).inputValue())).toBeCloseTo(5,0);
  expect(Number(await page.getByLabel('Ширина, % — фрагмент 2',{exact:true}).inputValue())).toBeCloseTo(40,0);
  await surface.focus();await page.keyboard.press('ArrowRight');
  expect(Number(await page.getByLabel('Слева, % — фрагмент 2',{exact:true}).inputValue())).toBeCloseTo(6,0);
  await page.getByRole('button',{name:'Удалить фрагмент 1',exact:true}).click();
  await expect(page.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true})).toHaveAttribute('aria-pressed','true');
  expect(Number(await page.getByLabel('Слева, % — фрагмент 1',{exact:true}).inputValue())).toBeCloseTo(6,0);
  await page.getByRole('button',{name:'Весь кадр — фрагмент 1',exact:true}).click();
  await expect(page.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true})).toBeFocused();
  await expect(page.getByLabel('Кадр после обрезки',{exact:true})).toHaveCount(0);
  await expect(page.getByLabel('Слева, % — фрагмент 1',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты'}).click();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:unknown}[]}}).fixture.calls);
  expect(calls.find(c=>c.command==='media_begin')?.cut).toEqual({segments:[{start_ms:0,end_ms:1000}]});
});

test('prepared movie streams bounded ranges and publication requires one explicit confirmation',async({page})=>{
  const config=JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
  await page.route('http://127.0.0.1:1420/',async route=>{
    const response=await route.fetch(), html=await response.text();
    const hashes=Array.from(html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g))
      .filter(m=>!m[1].includes('src='))
      .map(m=>`'sha256-${createHash('sha256').update(m[2]).digest('base64')}'`);
    await route.fulfill({response,headers:{...response.headers(),'content-security-policy':`${config.app.security.csp}; script-src 'self' ${hashes.join(' ')}`}});
  });
  await page.reload();
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  const ranges:string[]=[];
  await page.route('http://media-preview.localhost/**',async route=>{
    const requested=route.request().headers()['range'] ?? '';
    ranges.push(requested);
    const match=/^bytes=(\d+)-(\d*)$/.exec(requested);
    const start=match ? Number(match[1]) : 0;
    const end=Math.min(videoFixture.length-1,start+511,match?.[2] ? Number(match[2]) : Infinity);
    await route.fulfill({status:206,headers:{'access-control-allow-origin':'*','content-type':'video/mp4','accept-ranges':'bytes','content-range':`bytes ${start}-${end}/${videoFixture.length}`,'cache-control':'no-store'},body:videoFixture.subarray(start,end+1)});
  });
  await page.evaluate(()=>{const f=(window as unknown as {fixture:{reviewReady:boolean;delayConfirm:boolean}}).fixture;f.reviewReady=true;f.delayConfirm=true;});
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('2');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'prepared.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты'}).click();
  const video=page.getByLabel('Готовый ролик',{exact:true});
  await expect(video).toBeVisible();
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  await video.evaluate(v=>{(v as HTMLVideoElement).currentTime=1.5;});
  await expect.poll(()=>video.evaluate(v=>!(v as HTMLVideoElement).seeking && (v as HTMLVideoElement).readyState>=2)).toBe(true);
  expect(await video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeCloseTo(1.5,2);
  expect(ranges.length).toBeGreaterThan(1);
  const commands=()=>page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.map(c=>c.command));
  expect(await commands()).not.toContain('media_confirm');
  await expect(page.getByLabel('Конец фрагмента 1, с',{exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Сохранить готовый ролик',exact:true}).click();
  await expect.poll(()=>commands().then(c=>c.filter(v=>v==='media_confirm').length)).toBe(1);
  await expect(page.getByText('Передача ролика в библиотеку: prepared.mp4',{exact:true})).toBeVisible();
  await expect(video).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {fixture:{confirmRelease:()=>void}}).fixture.confirmRelease());
  await expect(page.getByText('Ролик сохранён в библиотеке: prepared.mp4',{exact:true})).toBeVisible();
  expect((await commands()).filter(c=>c==='media_confirm')).toHaveLength(1);
});

test('closing a ready review cancels staging without publication even if the player cannot decode it',async({page})=>{
  await page.route('http://media-preview.localhost/**',route=>route.fulfill({status:404,body:''}));
  await page.evaluate(()=>(window as unknown as {fixture:{reviewReady:boolean}}).fixture.reviewReady=true);
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'prepared.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты'}).click();
  await expect(page.getByText('Не удалось воспроизвести готовый ролик в этом окне.',{exact:false})).toBeVisible();
  await page.keyboard.press('Escape');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe('cancelled');
  const commands=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.map(c=>c.command));
  expect(commands).not.toContain('media_confirm');
});

test('lost confirmation reply recovers the published outcome without automatic second confirmation',async({page})=>{
  await page.route('http://media-preview.localhost/**',route=>route.fulfill({status:200,contentType:'video/mp4',body:videoFixture}));
  await page.evaluate(()=>{const f=(window as unknown as {fixture:{reviewReady:boolean;lostConfirm:boolean}}).fixture;f.reviewReady=true;f.lostConfirm=true;});
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'prepared.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты'}).click();
  await page.getByRole('button',{name:'Сохранить готовый ролик',exact:true}).click();
  await expect(page.getByText('Ролик сохранён в библиотеке: prepared.mp4',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Сохранить готовый ролик',exact:true})).toHaveCount(0);
  const commands=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.map(c=>c.command));
  expect(commands.filter(c=>c==='media_confirm')).toHaveLength(1);
});


test('record and pause capture ordered source intervals with crop snapshots and preserve the manual draft',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Начало фрагмента 1, с',{exact:true}).fill('0.1');
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'record.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const video=page.getByLabel('Исходное видео',{exact:true}), mode=page.getByLabel('Способ выбора фрагментов');
  await mode.selectOption('true');
  const record=page.getByRole('button',{name:'Записать фрагмент',exact:true});
  await expect(record).toBeEnabled();
  await page.getByLabel('Ширина, % — запись',{exact:true}).fill('50');
  await page.getByLabel('Позиция в исходном видео').evaluate(input=>{(input as HTMLInputElement).value='0.25';input.dispatchEvent(new Event('input',{bubbles:true}));});
  await expect(record).toBeEnabled();
  await record.click();
  await expect(mode).toBeDisabled();
  await expect(page.getByLabel('Позиция в исходном видео')).toBeDisabled();
  await expect(page.getByLabel('Ширина, % — запись',{exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Подготовить выбранные фрагменты'})).toBeDisabled();
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeGreaterThan(.55);
  await page.getByRole('button',{name:'Пауза записи',exact:true}).click();
  const firstEnd=await video.evaluate(v=>Math.round((v as HTMLVideoElement).currentTime*1000));
  await expect(mode).toBeEnabled();
  await page.getByLabel('Слева, % — запись',{exact:true}).fill('50');
  await page.getByLabel('Позиция в исходном видео').evaluate(input=>{(input as HTMLInputElement).value='1.25';input.dispatchEvent(new Event('input',{bubbles:true}));});
  await expect(record).toBeEnabled();await record.click();
  // The second interval finishes automatically at the source end.
  await expect(page.getByRole('button',{name:'Пауза записи',exact:true})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Удалить фрагмент 2',exact:true})).toBeVisible();
  await mode.selectOption('false');
  await expect(page.getByLabel('Начало фрагмента 1, с',{exact:true})).toHaveValue('0.1');
  await mode.selectOption('true');
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты'}).click();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:{segments:{start_ms:number;end_ms:number;crop:unknown}[]}}[]}}).fixture.calls);
  const parts=calls.find(c=>c.command==='media_begin')!.cut!.segments;
  expect(parts).toHaveLength(2);
  expect(parts[0]).toEqual({start_ms:250,end_ms:firstEnd,crop:{x:0,y:0,width:.5,height:1}});
  expect(parts[1]).toEqual({start_ms:1250,end_ms:2000,crop:{x:.5,y:0,width:.5,height:1}});
});

test('failed playback and unexpected seeking discard an unfinished recording; an empty recorded list cannot be imported',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'record.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const mode=page.getByLabel('Способ выбора фрагментов');await mode.selectOption('true');
  const video=page.getByLabel('Исходное видео',{exact:true}), record=page.getByRole('button',{name:'Записать фрагмент',exact:true});
  await expect(record).toBeEnabled();
  await video.evaluate(v=>{Object.assign(v,{play:()=>Promise.reject(new Error('decoder unavailable'))});});
  await record.click();
  await expect(page.getByText('Не удалось начать воспроизведение. Фрагмент не добавлен.',{exact:true})).toBeVisible();
  await expect(mode).toBeEnabled();
  await video.evaluate(v=>{Object.assign(v,{play:HTMLMediaElement.prototype.play});});
  await record.click();
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeGreaterThan(.2);
  await video.evaluate(v=>{(v as HTMLVideoElement).currentTime=1.5;});
  await expect(page.getByText('Позиция изменилась во время записи; незавершённый фрагмент не добавлен.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Подготовить выбранные фрагменты'})).toBeDisabled();
  await expect(mode).toBeEnabled();
  await expect(record).toBeEnabled();await record.click();
  await expect(page.getByRole('button',{name:'Удалить фрагмент 1',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Удалить фрагмент 1',exact:true}).click();
  await expect(mode).toBeFocused();
  await expect(page.getByRole('button',{name:'Подготовить выбранные фрагменты'})).toBeDisabled();
});


test('recording stops at the source-time budget even when playback runs faster',async({page})=>{
  test.setTimeout(45000);
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'long.mp4',mimeType:'video/mp4',buffer:longFixture});
  await page.getByLabel('Способ выбора фрагментов').selectOption('true');
  const video=page.getByLabel('Исходное видео',{exact:true});
  const record=page.getByRole('button',{name:'Записать фрагмент',exact:true});
  await expect(record).toBeEnabled();
  await video.evaluate(v=>{(v as HTMLVideoElement).playbackRate=16;});
  await record.click();
  await expect(page.getByRole('button',{name:'Пауза записи',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Пауза записи',exact:true})).toHaveCount(0,{timeout:20000});
  await expect(record).toBeDisabled();
  await expect(page.getByText('Выбрано: 180.000 с.',{exact:true})).toBeVisible();
  await expect(page.getByText('Конвертация для LCD: long.mp4',{exact:true})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;cut?:{segments:{start_ms:number;end_ms:number}[]}}[]}}).fixture.calls);
  const parts=calls.find(c=>c.command==='media_begin')!.cut!.segments;
  expect(parts).toHaveLength(1);expect(parts[0].start_ms).toBe(0);expect(parts[0].end_ms).toBe(180000);
});

async function openSavedEditor(page:import('@playwright/test').Page) {
  const config=JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
  await page.route('http://127.0.0.1:1420/',async route=>{
    const response=await route.fetch(), html=await response.text();
    const hashes=Array.from(html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)).filter(m=>!m[1].includes('src=')).map(m=>`'sha256-${createHash('sha256').update(m[2]).digest('base64')}'`);
    await route.fulfill({response,headers:{...response.headers(),'content-security-policy':`${config.app.security.csp}; script-src 'self' ${hashes.join(' ')}`}});
  });
  await page.reload();
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await page.route('http://media-source.localhost/**',route=>route.fulfill({status:200,headers:{'Access-Control-Allow-Origin':'*'},contentType:'video/mp4',body:videoFixture}));
  await page.route('http://media-preview.localhost/**',route=>route.fulfill({status:200,headers:{'Access-Control-Allow-Origin':'*'},contentType:'video/mp4',body:videoFixture}));
  await page.evaluate(()=>{
    const f=(window as any).fixture;
    f.clips=[{key:'c'.repeat(32),name:'Original.mp4',fps:24,frames:48,encoded_bytes:100}];
    f.collections={version:1,revision:7,images:[],watermarks:[],backgrounds:[{kind:'video',key:'c'.repeat(32)}]};
  });
  await page.getByRole('button',{name:'Обновить ролики',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать выбранный ролик',exact:true}).click();
  const editor=page.getByRole('region',{name:'Редактор сохранённого ролика'});
  await expect(editor.getByLabel('Конец фрагмента 1, с',{exact:true})).toBeVisible();
  await editor.getByLabel('Начало фрагмента 1, с',{exact:true}).fill('0.5');
  await editor.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1.5');
  return editor;
}

test('saved video edits through native source and replaces only gallery membership',async({page})=>{
  const editor=await openSavedEditor(page);
  await editor.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true}).click();
  const video=editor.locator('video').first();
  await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  const canvas=editor.locator('canvas').last();
  await expect.poll(()=>canvas.evaluate(c=>{const p=(c as HTMLCanvasElement).getContext('2d')!.getImageData(240,240,1,1).data;return p[0]>200 && p[1]<30;})).toBe(true);

  await editor.getByLabel('Ширина, % — фрагмент 1',{exact:true}).fill('50');
  await editor.getByRole('button',{name:'Подготовить изменения ролика',exact:true}).click();
  await editor.getByRole('button',{name:'Сохранить готовый ролик',exact:true}).click();
  await expect(page.getByText('Ролик заменён в коллекции. Исходный файл сохранён для других ссылок.',{exact:true})).toBeVisible();
  await expect(page.getByRole('combobox',{name:'Сохранённый ролик',exact:true})).toHaveValue('a'.repeat(32));
  const f=await page.evaluate(()=>{const f=(window as any).fixture;return {collections:f.collections,clips:f.clips,calls:f.calls};});
  expect(f.collections.backgrounds).toEqual([{kind:'video',key:'a'.repeat(32)}]);
  expect(f.clips[0].key).toBe('c'.repeat(32));
  const prepare=f.calls.find((c:any)=>c.command==='media_source_prepare');
  expect(prepare.cut.segments).toEqual([{start_ms:500,end_ms:1500,crop:{x:0,y:0,width:0.5,height:1}}]);
  expect(f.calls.some((c:any)=>['media_begin','media_append','delete_prepared_clip','set_lcd_media'].includes(c.command))).toBe(false);
  await expect(page.getByRole('button',{name:'Редактировать выбранный ролик',exact:true})).toBeFocused();
});

test('gallery revision conflict keeps both files and does not overwrite concurrent membership',async({page})=>{
  const editor=await openSavedEditor(page);
  await page.evaluate(()=>{const f=(window as any).fixture;f.collections.revision++;f.collections.watermarks=['concurrent'];});
  await editor.getByRole('button',{name:'Подготовить изменения ролика',exact:true}).click();
  await editor.getByRole('button',{name:'Сохранить готовый ролик',exact:true}).click();
  await expect(page.getByText(/Новый ролик сохранён в каталоге, но замена в коллекции не подтверждена/)).toBeVisible();
  const state=await page.evaluate(()=>{const f=(window as any).fixture;return {collections:f.collections,phase:f.job.phase};});
  expect(state.collections.revision).toBe(8);
  expect(state.collections.watermarks).toEqual(['concurrent']);
  expect(state.collections.backgrounds).toEqual([{kind:'video',key:'c'.repeat(32)}]);
  expect(state.phase).toBe('complete');
});

test('AutoFill stretches the full selected source and sends native stretch placement',async({page})=>{
  await page.getByLabel('Размещение видео').selectOption('stretch');
  await expect(page.getByText('AutoFill заполняет экран всем кадром без обрезки; пропорции могут измениться.',{exact:true})).toBeVisible();
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('2');
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'wide.mp4',mimeType:'video/mp4',buffer:splitFixture});
  await page.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true}).click();
  const canvas=page.getByRole('region',{name:'Импорт ролика'}).locator('canvas').last();
  await expect.poll(()=>canvas.evaluate(c=>{
    const context=(c as HTMLCanvasElement).getContext('2d')!;
    const left=context.getImageData(24,24,1,1).data,right=context.getImageData(456,24,1,1).data;
    return left[0]>200 && left[1]<30 && right[2]>200 && right[0]<30;
  })).toBe(true);
  await page.getByRole('button',{name:'Подготовить выбранные фрагменты',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;fit?:string}[]}}).fixture.calls.find(c=>c.command==='media_begin')?.fit)).toBe('stretch');
});


test('whole-video color correction previews source pixels, validates and requires review',async({page})=>{
  await page.getByLabel('Цветокоррекция видео',{exact:true}).check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'colors.mp4',mimeType:'video/mp4',buffer:splitFixture});
  const submit=page.getByRole('button',{name:'Подготовить ролик',exact:true});
  await expect(submit).toBeEnabled();
  await expect(page.getByLabel('Выбрать фрагменты видео',{exact:true})).not.toBeChecked();
  const preview=page.getByLabel('Кадр после обрезки',{exact:true});
  const sample=(x:number,y:number)=>preview.evaluate((canvas,p)=>Array.from((canvas as HTMLCanvasElement).getContext('2d')!.getImageData(p.x,p.y,1,1).data),{x,y});
  await expect.poll(async()=> (await sample(60,240))[0]).toBeGreaterThan(200);
  await page.getByLabel('Контраст видео — значение',{exact:true}).fill('32');
  await page.getByLabel('Яркость видео — значение',{exact:true}).fill('255');
  await expect.poll(async()=> (await sample(60,240)).slice(0,3)).toEqual([255,255,255]);
  expect((await sample(240,5)).slice(0,3)).toEqual([0,0,0]);
  await page.getByLabel('Яркость видео — значение',{exact:true}).fill('256');
  await expect(submit).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('от 0 до 255');
  await page.getByLabel('Яркость видео — значение',{exact:true}).fill('');
  await expect(submit).toBeDisabled();
  await page.getByRole('button',{name:'Сбросить цветокоррекцию',exact:true}).click();
  await expect.poll(async()=> (await sample(60,240))[1]).toBeLessThan(30);
  await page.getByLabel('Яркость видео — значение',{exact:true}).fill('128');
  await page.evaluate(()=>{(window as any).fixture.reviewReady=true;});
  await submit.click();
  await expect(page.getByRole('button',{name:'Сохранить готовый ролик',exact:true})).toBeVisible();
  await expect(page.getByLabel('Яркость видео — значение',{exact:true})).toBeDisabled();
  const calls=await page.evaluate(()=>(window as any).fixture.calls);
  expect(calls.find((c:any)=>c.command==='media_begin')).toEqual({command:'media_begin',fit:'contain',adjustments:{brightness:128,contrast:0}});
  expect(calls.some((c:any)=>c.command==='media_confirm')).toBe(false);
  await page.getByRole('button',{name:'Отменить импорт',exact:true}).click();
  await expect(submit).toBeEnabled();
});

test('gallery effects retain cuts and send the previewed values',async({page})=>{
  const editor=await openSavedEditor(page);
  await editor.getByLabel('Цветокоррекция видео',{exact:true}).check();
  await editor.getByLabel('Контраст видео — значение',{exact:true}).fill('64');
  await editor.getByLabel('Яркость видео — значение',{exact:true}).fill('255');
  const preview=editor.getByLabel('Кадр после обрезки',{exact:true});
  await expect.poll(()=>preview.evaluate(canvas=>Array.from((canvas as HTMLCanvasElement).getContext('2d')!.getImageData(240,240,1,1).data).slice(0,3))).toEqual([255,255,255]);
  await editor.getByRole('button',{name:'Подготовить изменения ролика',exact:true}).click();
  await expect(editor.getByRole('button',{name:'Сохранить готовый ролик',exact:true})).toBeVisible();
  const calls=await page.evaluate(()=>(window as any).fixture.calls);
  const request=calls.find((c:any)=>c.command==='media_source_prepare');
  expect(request.adjustments).toEqual({contrast:64,brightness:255});
  expect(request.cut.segments).toEqual([{start_ms:500,end_ms:1500}]);
});


test('VideoEdit inspects before fractional selection and requires explicit review/save',async({page})=>{
  await page.getByLabel('Один диапазон VideoEdit',{exact:true}).check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'range.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const prepare=page.getByRole('button',{name:'Подготовить диапазон VideoEdit',exact:true});
  await expect(prepare).toBeEnabled();
  const end=page.getByLabel('Конец VideoEdit, с',{exact:true});
  expect(Number(await end.inputValue())).toBeCloseTo(2-1/24,12);
  await end.fill('0.01');await expect(prepare).toBeDisabled();
  await end.fill('2');await expect(prepare).toBeDisabled();
  await page.getByLabel('Начало VideoEdit, с',{exact:true}).fill('1.101001');
  await end.fill('1.600001');
  await page.getByLabel('Цветокоррекция видео',{exact:true}).check();
  await page.getByLabel('Яркость видео — значение',{exact:true}).fill('12');
  const commands=()=>page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;range?:unknown;cut?:unknown;adjustments?:unknown}[]}}).fixture.calls);
  expect((await commands()).map(c=>c.command)).toEqual(['media_begin','media_append','media_inspect']);
  await prepare.click();
  const save=page.getByRole('button',{name:'Сохранить готовый ролик',exact:true});
  await expect(save).toBeEnabled();
  expect((await commands()).find(c=>c.command==='media_prepare_inspected')).toEqual({command:'media_prepare_inspected',fit:'contain',range:{startSeconds:1.101001,endSeconds:1.600001},adjustments:{contrast:0,brightness:12}});
  expect((await commands()).some(c=>c.command==='media_confirm')).toBe(false);
  await save.click();
  await expect(page.getByText('Ролик сохранён в библиотеке: range.mp4',{exact:true})).toBeVisible();
});

test('VideoEdit recovers a lost inspection reply and cancellation releases the upload',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{lostInspect:boolean}}).fixture.lostInspect=true;});
  await page.getByLabel('Один диапазон VideoEdit',{exact:true}).check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'range.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await expect(page.getByRole('button',{name:'Подготовить диапазон VideoEdit',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.some(c=>c.command==='media_cancel'))).toBe(false);
  await page.getByRole('button',{name:'Отменить импорт',exact:true}).click();
  await expect(page.getByLabel('Импортировать GIF/MP4')).toBeEnabled();
  await expect(page.getByRole('button',{name:'Подготовить диапазон VideoEdit',exact:true})).toHaveCount(0);
});

test('VideoEdit unknown FPS blocks preparation and close during inspection cancels',async({page})=>{
  await page.evaluate(()=>{const f=(window as unknown as {fixture:{metadata:{averageFrameRate:null;nominalFrameRate:null}}}).fixture;f.metadata.averageFrameRate=null;f.metadata.nominalFrameRate=null;});
  await page.getByLabel('Один диапазон VideoEdit',{exact:true}).check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'unknown.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await expect(page.getByText(/Нет допустимой длительности/)).toBeVisible();
  await expect(page.getByRole('button',{name:'Подготовить диапазон VideoEdit',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Отменить импорт',exact:true}).click();
  await page.evaluate(()=>{(window as unknown as {fixture:{delayInspect:boolean}}).fixture.delayInspect=true;});
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'delayed.mp4',mimeType:'video/mp4',buffer:videoFixture});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{inspectRelease:unknown}}).fixture.inspectRelease!==null)).toBe(true);
  await page.keyboard.press('Escape');
  await page.evaluate(()=>{(window as unknown as {fixture:{inspectRelease:()=>void}}).fixture.inspectRelease();});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe('cancelled');
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.some(c=>c.command==='media_prepare_inspected'))).toBe(false);
});

test('gallery VideoEdit uses managed metadata and range without uploading again',async({page})=>{
  const editor=await openSavedEditor(page);
  await editor.getByLabel('Один диапазон VideoEdit',{exact:true}).check();
  await editor.getByLabel('Начало VideoEdit, с',{exact:true}).fill('0.101001');
  await editor.getByLabel('Конец VideoEdit, с',{exact:true}).fill('1.600001');
  await editor.getByRole('button',{name:'Подготовить изменения ролика',exact:true}).click();
  await expect(editor.getByRole('button',{name:'Сохранить готовый ролик',exact:true})).toBeEnabled();
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string;range?:unknown;cut?:unknown}[]}}).fixture.calls);
  expect(calls.find(c=>c.command==='media_source_prepare')).toMatchObject({range:{startSeconds:0.101001,endSeconds:1.600001},cut:null});
  expect(calls.some(c=>['media_begin','media_inspect','media_confirm'].includes(c.command))).toBe(false);
});


test('gallery edit restores focus after the reopened import panel refreshes the catalog',async({page})=>{
  const editor=await openSavedEditor(page);
  await page.evaluate(()=>{(window as unknown as {fixture:{delayPostEditCatalog:boolean}}).fixture.delayPostEditCatalog=true;});
  await editor.getByRole('button',{name:'Подготовить изменения ролика',exact:true}).click();
  await editor.getByRole('button',{name:'Сохранить готовый ролик',exact:true}).click();
  await expect(editor).toHaveCount(0);
  await expect(page.getByLabel('Импортировать GIF/MP4')).toBeEnabled();
  const edit=page.getByRole('button',{name:'Редактировать выбранный ролик',exact:true});
  // Implementations may avoid the redundant refresh or wait for it; either
  // way, focus must survive the new import panel's completed-job readback.
  const delayed=await page.evaluate(()=>(window as unknown as {fixture:{catalogRelease:unknown}}).fixture.catalogRelease!==null);
  if(delayed) {
    await expect(edit).toBeDisabled();
    await page.evaluate(()=>{(window as unknown as {fixture:{catalogRelease:()=>void}}).fixture.catalogRelease();});
  }
  await expect(edit).toBeEnabled();
  await expect(edit).toBeFocused();
});


async function mediaLanguage(page:Page,language:string) {
  await page.evaluate(language=>{(window as unknown as {fixture:{language:string}}).fixture.language=language;},language);
  await expect(page.getByRole('region',{name:language==='en-US'?'Video import':'Импорт ролика',exact:true})).toBeVisible();
}

for(const language of ['en-US','ru-RU'])test(`failed cancel permits explicit retry after status readback (${language})`,async({page})=>{
  await mediaLanguage(page,language);
  const english=language==='en-US';
  await page.evaluate(()=>{(window as unknown as {fixture:{cancelFailure:boolean}}).fixture.cancelFailure=true;});
  await page.getByLabel(english?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:'Настройки.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const cancel=page.getByRole('button',{name:english?'Cancel import':'Отменить импорт',exact:true});
  await cancel.click();
  await expect(page.getByRole('region',{name:english?'Video import':'Импорт ролика',exact:true}).getByRole('alert')).toHaveText('Настройки');
  await expect(cancel).toBeEnabled();
  const before=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[];job:{id:string;phase:string}}}).fixture);
  expect(before.job.phase).toBe('encoding');
  expect(before.calls.filter(c=>c.command==='media_cancel')).toHaveLength(1);
  await cancel.click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe('cancelled');
  const after=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[];job:{id:string}}}).fixture);
  expect(after.job.id).toBe(before.job.id);
  expect(after.calls.filter(c=>c.command==='media_cancel')).toHaveLength(2);
  expect(after.calls.some(c=>c.command==='media_confirm')).toBe(false);
});


test('cancel retry waits for readable matching state and stays blocked for publishing',async({page})=>{
  await mediaLanguage(page,'en-US');
  await page.getByLabel('Import GIF/MP4',{exact:true}).setInputFiles({name:'Настройки.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const original=await page.evaluate(()=>{
    const f=(window as unknown as {fixture:{job:{id:string;phase:string};cancelFailure:boolean;failStatus:boolean}}).fixture;
    f.cancelFailure=true;f.failStatus=true;return {...f.job};
  });
  const cancel=page.getByRole('button',{name:'Cancel import',exact:true}),check=page.getByRole('button',{name:'Check import',exact:true});
  await cancel.click();
  await expect(page.getByRole('region',{name:'Video import',exact:true}).getByRole('alert')).toHaveText('Настройки');
  await expect(cancel).toBeDisabled();
  await page.evaluate(()=>{
    const f=(window as unknown as {fixture:{failStatus:boolean;job:{id:string}}}).fixture;
    f.failStatus=false;f.job.id='f'.repeat(32);
  });
  await check.click();await expect(cancel).toBeDisabled();
  await page.evaluate(original=>{
    const f=(window as unknown as {fixture:{job:unknown}}).fixture;
    f.job={...original,phase:'publishing'};
  },original);
  await check.click();await expect(cancel).toBeDisabled();
  await page.evaluate(original=>{(window as unknown as {fixture:{job:unknown}}).fixture.job=original;},original);
  await check.click();await expect(cancel).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.filter(c=>c.command==='media_cancel').length)).toBe(1);
  await cancel.click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe('cancelled');
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.filter(c=>c.command==='media_cancel').length)).toBe(2);
});


for(const context of ['upload','prepare','confirm'] as const)for(const language of ['en-US','ru-RU']) {
  test(`import ${context} recovery copy follows language and retries explicitly (${language})`,async({page})=>{
    await page.route('http://media-preview.localhost/**',route=>route.fulfill({status:200,contentType:'video/mp4',body:videoFixture}));
    await mediaLanguage(page,language);
    const english=language==='en-US',fileName='Настройки.mp4';
    const diagnostic=context==='confirm'?'Настройки':'probe {0} <diagnostic>';
    if(context==='upload') {
      await page.getByLabel(english?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:'empty.mp4',mimeType:'video/mp4',buffer:Buffer.alloc(0)});
      await expect(page.getByRole('region',{name:english?'Video import':'Импорт ролика',exact:true}).getByRole('alert')).toHaveText(english?'Select a nonempty GIF or MP4 up to 256 MiB.':'Выберите непустой GIF или MP4 размером до 256 MiB.');
    } else if(context==='prepare') {
      await page.getByLabel(english?'Single VideoEdit range':'Один диапазон VideoEdit',{exact:true}).check();
    } else {
      await page.getByLabel(english?'Select video segments':'Выбрать фрагменты видео',{exact:true}).check();
      await page.getByLabel(english?'Segment end 1, s':'Конец фрагмента 1, с',{exact:true}).fill('1');
      await page.getByRole('button',{name:english?'Choose segment region 1':'Выбрать область фрагмента 1',exact:true}).click();
      const region=page.getByRole('group',{name:english?'Segment region 1':'Область фрагмента 1',exact:true});
      for(const [en,ru] of [['Left','Слева'],['Top','Сверху'],['Width','Ширина'],['Height','Высота']]) {
        await expect(region.getByLabel(english?`${en}, % — segment 1`:`${ru}, % — фрагмент 1`,{exact:true})).toBeVisible();
      }
    }
    await page.evaluate(({context,diagnostic})=>{
      const f=(window as unknown as {fixture:{failCommand:string;diagnostic:string;reviewReady:boolean}}).fixture;
      f.reviewReady=true;f.diagnostic=diagnostic;
      f.failCommand=context==='upload'?'media_append':context==='prepare'?'media_prepare_inspected':'media_confirm';
    },{context,diagnostic});
    await page.getByLabel(english?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:fileName,mimeType:'video/mp4',buffer:videoFixture});
    const action=(en:boolean)=>context==='prepare'?(en?'Prepare VideoEdit range':'Подготовить диапазон VideoEdit'):(en?'Save prepared video':'Сохранить готовый ролик');
    if(context==='prepare') {
      await page.getByLabel(english?'VideoEdit start, s':'Начало VideoEdit, с',{exact:true}).fill('0.25');
      await page.getByLabel(english?'VideoEdit end, s':'Конец VideoEdit, с',{exact:true}).fill('1.5');
      await page.getByRole('button',{name:action(english),exact:true}).click();
    } else if(context==='confirm') {
      await page.getByRole('button',{name:english?'Prepare selected segments':'Подготовить выбранные фрагменты',exact:true}).click();
      await page.getByRole('button',{name:action(english),exact:true}).click();
    }
    const expected=(en:boolean)=>context==='upload'?
      `${diagnostic} ${en?'Check the import status and library before uploading again.':'Проверьте состояние импорта и библиотеку перед повторной загрузкой.'}`:
      context==='prepare'?`${diagnostic} ${en?'Check the preparation status.':'Проверьте состояние подготовки.'}`:
      `${diagnostic} ${en?'Check the import status before confirming again.':'Проверьте состояние импорта перед повторным подтверждением.'}`;
    const panel=(en:boolean)=>page.getByRole('region',{name:en?'Video import':'Импорт ролика',exact:true});
    await expect(panel(english).getByRole('alert')).toHaveText(expected(english));
    const before=await page.evaluate(()=>(window as unknown as {fixture:{job:{id:string};calls:{command:string}[]}}).fixture);
    for(const next of [english?'ru-RU':'en-US',language]) {
      await mediaLanguage(page,next);await expect(panel(next==='en-US').getByRole('alert')).toHaveText(expected(next==='en-US'));
    }
    const afterSwitch=await page.evaluate(()=>(window as unknown as {fixture:{job:{id:string;name:string};calls:{command:string}[]}}).fixture);
    expect(afterSwitch.job.id).toBe(before.job.id);expect(afterSwitch.job.name).toBe(fileName);
    expect(afterSwitch.calls.filter(c=>c.command==='media_begin')).toHaveLength(1);
    expect(afterSwitch.calls.filter(c=>c.command==='media_confirm')).toHaveLength(context==='confirm'?1:0);
    if(context==='upload') {
      await page.getByLabel(english?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:fileName,mimeType:'video/mp4',buffer:videoFixture});
      await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe('encoding');
    } else {
      if(context==='prepare') {
        await expect(page.getByLabel(english?'VideoEdit start, s':'Начало VideoEdit, с',{exact:true})).toHaveValue('0.25');
        await expect(page.getByLabel(english?'VideoEdit end, s':'Конец VideoEdit, с',{exact:true})).toHaveValue('1.5');
      }
      await page.getByRole('button',{name:action(english),exact:true}).click();
      await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string}}}).fixture.job.phase)).toBe(context==='confirm'?'complete':'review_ready');
    }
    const result=await page.evaluate(()=>(window as unknown as {fixture:{job:{id:string;name:string};calls:{command:string}[]}}).fixture);
    expect(result.job.name).toBe(fileName);
    expect(result.calls.filter(c=>c.command==='media_confirm')).toHaveLength(context==='confirm'?2:0);
    expect(result.calls.filter(c=>c.command==='media_begin')).toHaveLength(context==='upload'?2:1);
  });
}


test('pending cancellation blocks VideoEdit prepare even after successful status polls',async({page})=>{
  await mediaLanguage(page,'en-US');
  await page.getByLabel('Single VideoEdit range',{exact:true}).check();
  await page.getByLabel('Import GIF/MP4',{exact:true}).setInputFiles({name:'Настройки.mp4',mimeType:'video/mp4',buffer:videoFixture});
  const prepare=page.getByRole('button',{name:'Prepare VideoEdit range',exact:true});
  await expect(prepare).toBeEnabled();
  await page.evaluate(()=>{(window as unknown as {fixture:{delayCancel:boolean}}).fixture.delayCancel=true;});
  const cancel=page.getByRole('button',{name:'Cancel import',exact:true});
  await cancel.click();
  const baseline=await page.evaluate(()=>(window as unknown as {fixture:{statusReads:number}}).fixture.statusReads);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{statusReads:number}}).fixture.statusReads)).toBeGreaterThan(baseline+1);
  await expect(cancel).toBeDisabled();await expect(prepare).toBeDisabled();
  await page.evaluate(()=>{
    const f=(window as unknown as {fixture:{delayCancel:boolean;cancelRelease:null|(()=>void)}}).fixture;
    f.delayCancel=false;f.cancelRelease?.();
  });
  await expect(page.getByLabel('Import GIF/MP4',{exact:true})).toBeEnabled();
  const result=await page.evaluate(()=>(window as unknown as {fixture:{job:{phase:string};calls:{command:string}[]}}).fixture);
  expect(result.job.phase).toBe('cancelled');
  expect(result.calls.filter(c=>c.command==='media_cancel')).toHaveLength(1);
  expect(result.calls.some(c=>['media_prepare_inspected','media_confirm'].includes(c.command))).toBe(false);
});

test('video crop capture loss cancels the pending rectangle and a fresh gesture works',async({page})=>{
  await page.getByLabel('Выбрать фрагменты видео').check();
  await page.getByLabel('Импортировать GIF/MP4').setInputFiles({name:'split.mp4',mimeType:'video/mp4',buffer:splitFixture});
  await page.getByLabel('Конец фрагмента 1, с',{exact:true}).fill('1');
  await page.getByRole('button',{name:'Выбрать область фрагмента 1',exact:true}).click();
  const surface=page.getByRole('button',{name:'Выбор области видео',exact:true});
  await expect(surface).toBeVisible();await surface.scrollIntoViewIfNeeded();const box=(await surface.boundingBox())!;
  await surface.evaluate(element=>element.addEventListener('pointerdown',event=>element.setAttribute('data-last-pointer',String((event as PointerEvent).pointerId)),{once:true}));
  const left=page.getByLabel('Слева, % — фрагмент 1',{exact:true});const before=await left.inputValue();
  await page.mouse.move(box.x+box.width*.1,box.y+box.height*.1);await page.mouse.down();
  const id=Number(await surface.getAttribute('data-last-pointer'));
  await surface.evaluate((element,id)=>element.releasePointerCapture(id),id);
  await page.mouse.move(box.x+box.width*.8,box.y+box.height*.8);await page.mouse.up();
  await expect(left).toHaveValue(before);
  await page.mouse.move(box.x+box.width*.2,box.y+box.height*.2);await page.mouse.down();
  await page.mouse.move(box.x+box.width*.7,box.y+box.height*.7);await page.mouse.up();
  expect(Number(await left.inputValue())).toBeCloseTo(20,0);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls.filter(c=>['media_begin','media_confirm'].includes(c.command)))).toEqual([]);
});

for(const language of ['en-US','ru-RU']) {
  test(`source notices follow language without replaying recording (${language})`,async({page})=>{
    await mediaLanguage(page,language);const en=language==='en-US';
    await page.getByLabel(en?'Select video segments':'Выбрать фрагменты видео',{exact:true}).check();
    await page.getByLabel(en?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:'Настройки {0}.mp4',mimeType:'video/mp4',buffer:videoFixture});
    await page.getByLabel(en?'Segment selection method':'Способ выбора фрагментов').selectOption('true');
    const video=page.getByLabel(en?'Source video':'Исходное видео',{exact:true});
    const record=()=>page.getByRole('button',{name:language==='en-US'?'Record segment':'Записать фрагмент',exact:true});
    await expect(record()).toBeEnabled();
    await video.evaluate(v=>{Object.assign(v,{play:()=>Promise.reject(new Error('decoder unavailable'))});});
    await record().click();
    const failure=(english:boolean)=>english?'Could not start playback. No segment was added.':'Не удалось начать воспроизведение. Фрагмент не добавлен.';
    await expect(page.getByRole('status').filter({hasText:failure(en)})).toBeVisible();
    await mediaLanguage(page,en?'ru-RU':'en-US');
    await expect(page.getByRole('status').filter({hasText:failure(!en)})).toBeVisible();
    await mediaLanguage(page,language);
    await video.evaluate(v=>{Object.assign(v,{play:HTMLMediaElement.prototype.play});});
    await record().click();
    await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeGreaterThan(.2);
    await page.getByRole('button',{name:en?'Pause recording':'Пауза записи',exact:true}).click();
    const added=(english:boolean)=>english?'Segment added.':'Фрагмент добавлен.';
    await expect(page.getByText(added(en),{exact:true})).toBeVisible();
    await mediaLanguage(page,en?'ru-RU':'en-US');
    await expect(page.getByText(added(!en),{exact:true})).toBeVisible();
    const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls);
    expect(calls.some(c=>['media_begin','media_confirm'].includes(c.command))).toBe(false);
  });
  test(`color preview failure follows language without preparing media (${language})`,async({page})=>{
    await mediaLanguage(page,language);const en=language==='en-US';
    await page.getByLabel(en?'Select video segments':'Выбрать фрагменты видео',{exact:true}).check();
    await page.getByLabel(en?'Import GIF/MP4':'Импортировать GIF/MP4',{exact:true}).setInputFiles({name:'Настройки.mp4',mimeType:'video/mp4',buffer:videoFixture});
    await expect(page.getByLabel(en?'Source video':'Исходное видео',{exact:true})).toBeVisible();
    await page.evaluate(()=>{CanvasRenderingContext2D.prototype.getImageData=()=>{throw new Error('readback unavailable');};});
    await page.getByLabel(en?'Video color correction':'Цветокоррекция видео',{exact:true}).check();
    await page.getByLabel(en?'Video brightness value':'Яркость видео — значение',{exact:true}).fill('1');
    const message=(english:boolean)=>english?'Color preview is unavailable. Check the prepared video after conversion.':'Цветовой preview недоступен. Проверьте готовый ролик после подготовки.';
    await expect(page.getByText(message(en),{exact:true})).toBeVisible();
    await mediaLanguage(page,en?'ru-RU':'en-US');
    await expect(page.getByText(message(!en),{exact:true})).toBeVisible();
    const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{command:string}[]}}).fixture.calls);
    expect(calls.some(c=>['media_begin','media_confirm'].includes(c.command))).toBe(false);
  });
}
