import {test,expect,type Page} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {mkdtempSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
async function setup(page:Page,videoUrl='data:video/mp4;base64,',language='ru-RU'){
  await page.addInitScript(({videoUrl,language})=>{
    localStorage.setItem('lconnect.interface-language',language);
    const f={snapshotUrls:[] as string[],revokedUrls:[] as string[],failSnapshot:false,holdSnapshot:false,releaseSnapshot:null as null|(()=>void),failRegion:false,lostRegion:false,regions:[] as unknown[],holdStatus:false,releaseStatus:null as null|(()=>void),failPause:false,failAdmit:false,failPrepare:false,deferFinish:false,failCancel:false,calls:[] as string[],capture:null as null|{id:string;phase:string;recordedMillis:number;frames:number;message:null;metadata:null;region:null|{x:number;y:number;width:number;height:number}},job:null as null|{id:string;phase:string;key:string|null;message:string|null},edit:null as unknown,collections:{version:1,revision:1,images:[],backgrounds:[],watermarks:[]} as unknown};
    const createUrl=URL.createObjectURL.bind(URL),revokeUrl=URL.revokeObjectURL.bind(URL);
    URL.createObjectURL=(blob:Blob|MediaSource)=>{const url=createUrl(blob);if(blob instanceof Blob && blob.type==='image/jpeg')f.snapshotUrls.push(url);return url;};
    URL.revokeObjectURL=(url:string)=>{f.revokedUrls.push(url);revokeUrl(url);};
    Object.assign(window,{recordingFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
      f.calls.push(command==='daemon_call'?String(args.method):command);
      if(command==='screen_record_begin'){f.capture={id:String(args.id),phase:'reserved',recordedMillis:0,frames:0,message:null,metadata:null,region:null};return {...f.capture};}
      if(command==='screen_record_start'){if(f.capture)f.capture.phase='setup';return;}
      if(command==='screen_record_status'){const snapshot=f.capture?{...f.capture}:null;if(f.holdStatus){f.holdStatus=false;await new Promise<void>(resolve=>{f.releaseStatus=resolve;});}return snapshot;}
      if(command==='screen_record_control'){if(typeof args.action==='object' && args.action!==null && 'set_region' in args.action){const region=args.action.set_region as {x:number;y:number;width:number;height:number};f.regions.push(region);if(f.failRegion)throw new Error('region rejected');if(f.capture)f.capture.region=region;if(f.lostRegion)throw new Error('lost region reply');return;}if(args.action==='pause' && f.failPause)throw new Error('pause rejected');if(f.capture)f.capture.phase=args.action==='pause'?'paused':args.action==='resume'?'recording':f.deferFinish?'finalizing':'ready';return;}
      if(command==='screen_record_snapshot'){
        if(f.holdSnapshot){f.holdSnapshot=false;await new Promise<void>(resolve=>{f.releaseSnapshot=resolve;});}
        if(f.failSnapshot)throw new Error('snapshot failed');
        const canvas=document.createElement('canvas');canvas.width=128;canvas.height=64;
        const context=canvas.getContext('2d')!;context.fillStyle='red';context.fillRect(0,0,64,64);context.fillStyle='blue';context.fillRect(64,0,64,64);
        const blob=await new Promise<Blob>(resolve=>canvas.toBlob(blob=>resolve(blob!),'image/jpeg'));
        return blob.arrayBuffer();
      }
      if(command==='screen_record_cancel'){if(f.capture)f.capture.phase='cancelled';return;}
      if(command==='screen_record_admit'){if(f.failAdmit)throw new Error('admission rejected');f.edit=args;f.job={id:String(args.id),phase:'uploading',key:null,message:null};return {...f.job};}
      if(command==='screen_record_prepare'){if(f.failPrepare){if(f.job){f.job.phase='failed';f.job.message='encoding rejected';}throw new Error('encoding rejected');}if(f.job)f.job.phase='review_ready';return;}
      if(command==='media_status')return f.job?{...f.job}:null;
      if(command==='media_confirm'){if(f.job){f.job.phase='complete';f.job.key='a'.repeat(32);}return;}
      if(command==='media_cancel'){if(f.failCancel){f.failCancel=false;throw new Error('lost cancel');}if(f.job)f.job.phase='cancelled';return;}
      if(command==='media_playback_url')return videoUrl;
      if(command==='daemon_call')switch(args.method){
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:language};
        case 'list_devices':return [{id:'A',display_name:'LCD A'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'get_lcd_media':return null;
        case 'list_prepared_clips':return [];
        case 'get_screen_collections':return {collections:f.collections,persistence:{saved:true,pending:false,last_error:null}};
        case 'set_screen_collections':f.collections=(args.params as {collections:unknown}).collections;return f.collections;
        default:return null;
      }
      return null;
    }}});
  },{videoUrl,language});
  await page.goto('/');await page.getByText(language==='en-US'?'Video library':'Библиотека роликов',{exact:true}).click();
  await page.getByRole('button',{name:language==='en-US'?'Record screen…':'Записать экран…',exact:true}).click();
}
test('capture pause resume review and explicit save add gallery membership',async({page})=>{
  await setup(page);
  await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  await page.getByRole('button',{name:'Продолжить запись',exact:true}).click();
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await expect(page.getByLabel('Исходное видео',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Подготовить запись для LCD',exact:true}).click();
  await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls.includes('media_confirm'))).toBe(false);
  await page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true}).click();
  await expect(page.getByText('Запись сохранена в библиотеке.',{exact:true})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls.filter(c=>c==='set_screen_collections').length)).toBe(1);
  await page.getByRole('button',{name:'Закрыть запись экрана',exact:true}).click();
  await expect(page.getByRole('region',{name:'Запись экрана',exact:true})).toHaveCount(0);
});
test('clear confirms cancellation and allows a fresh recording',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Очистить запись…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить очистку записи',exact:true}).click();
  await expect(page.getByRole('button',{name:'Выбрать экран или окно',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Закрыть запись экрана',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {recordingFixture:{capture:{phase:string}}}).recordingFixture.capture.phase)).toBe('cancelled');
});

test('automatic limit dialog cancels back to retained recording without reopening',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.evaluate(()=>{const f=(window as unknown as {recordingFixture:{capture:{phase:string;recordedMillis:number}}}).recordingFixture;f.capture.phase='limit_reached';f.capture.recordedMillis=180000;});
  const dialog=page.getByRole('dialog',{name:'Достигнут лимит записи'});
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true})).toBeFocused();
  const count=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls.filter(c=>c==='screen_record_status').length);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls.filter(c=>c==='screen_record_status').length)).toBeGreaterThan(count+2);
  await expect(dialog).not.toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls);
  expect(calls.filter(c=>c==='screen_record_control')).toHaveLength(1);expect(calls).not.toContain('screen_record_prepare');expect(calls).not.toContain('media_confirm');
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await expect(page.getByLabel('Исходное видео',{exact:true})).toBeVisible();
});
test('accepting automatic limit finishes once and still requires review and Save',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.evaluate(()=>{const f=(window as unknown as {recordingFixture:{capture:{phase:string;recordedMillis:number}}}).recordingFixture;f.capture.phase='limit_reached';f.capture.recordedMillis=180000;});
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await expect(page.getByLabel('Исходное видео',{exact:true})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls);
  expect(calls.filter(c=>c==='screen_record_control')).toHaveLength(2);
  expect(calls).not.toContain('screen_record_prepare');expect(calls).not.toContain('media_confirm');
});

test('recording trim and spatial crop validate before admission',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.evaluate(()=>{const f=(window as unknown as {recordingFixture:{capture:{metadata:unknown}}}).recordingFixture;f.capture.metadata={width:128,height:64,durationSeconds:2,averageFrameRate:{numerator:24,denominator:1},nominalFrameRate:{numerator:24,denominator:1}};});
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await page.getByLabel('Обрезать готовую запись').check();
  await page.getByLabel('Начало записи, с',{exact:true}).fill('0.25');
  await page.getByLabel('Конец записи, с',{exact:true}).fill('2.1');
  const prepare=page.getByRole('button',{name:'Подготовить запись для LCD',exact:true});
  await expect(prepare).toBeDisabled();
  await page.getByLabel('Конец записи, с',{exact:true}).fill('1.75');
  await page.getByLabel('Область X, %',{exact:true}).fill('50');
  await expect(prepare).toBeDisabled();
  await page.getByLabel('Ширина области, %',{exact:true}).fill('50');
  await expect(prepare).toBeEnabled();await prepare.click();
  const edit=await page.evaluate(()=>(window as unknown as {recordingFixture:{edit:{cut:unknown}}}).recordingFixture.edit);
  expect(edit.cut).toEqual({segments:[{start_ms:250,end_ms:1750,crop:{x:0.5,y:0,width:0.5,height:1}}]});
  await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
});

test('recording fragments retain independent crops and encode in chosen order',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{capture:{metadata:unknown}}}).recordingFixture.capture.metadata={width:128,height:64,durationSeconds:100,averageFrameRate:{numerator:24,denominator:1},nominalFrameRate:{numerator:24,denominator:1}};});
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await page.getByLabel('Обрезать готовую запись').check();
  await page.getByLabel('Ширина области, %',{exact:true}).fill('50');
  await page.getByRole('button',{name:'Добавить фрагмент записи',exact:true}).click();
  const prepare=page.getByRole('button',{name:'Подготовить запись для LCD',exact:true});
  await expect(prepare).toBeDisabled(); // 100+100 > 180
  await page.getByLabel('Начало записи, с',{exact:true}).fill('20');
  await page.getByLabel('Конец записи, с',{exact:true}).fill('40');
  await page.getByLabel('Область X, %',{exact:true}).fill('50');
  await page.getByLabel('Ширина области, %',{exact:true}).fill('50');
  await page.getByRole('button',{name:'Фрагмент раньше',exact:true}).click();
  await page.getByLabel('Фрагмент записи',{exact:true}).selectOption({label:'Фрагмент 2'});
  await expect(page.getByLabel('Область X, %',{exact:true})).toHaveValue('0');
  await expect(page.getByLabel('Ширина области, %',{exact:true})).toHaveValue('50');
  await page.getByLabel('Конец записи, с',{exact:true}).fill('10');
  await prepare.click();
  const edit=await page.evaluate(()=>(window as unknown as {recordingFixture:{edit:{cut:unknown}}}).recordingFixture.edit);
  expect(edit.cut).toEqual({segments:[{start_ms:20000,end_ms:40000,crop:{x:0.5,y:0,width:0.5,height:1}},{start_ms:0,end_ms:10000,crop:{x:0,y:0,width:0.5,height:1}}]});
});

test('recording source decodes, seeks, crops pixels per fragment and releases player on close',async({page})=>{
  const directory=mkdtempSync(join(tmpdir(),'recording-browser-'));
  try {
    const file=join(directory,'split.mp4');
    execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','color=red:s=128x64:r=24:d=2,drawbox=x=64:y=0:w=64:h=64:color=blue:t=fill','-c:v','libx264','-pix_fmt','yuv420p','-threads','1','-movflags','+faststart',file]);
    const bytes=readFileSync(file);
    await page.route('**/recording-fixture.mp4',async route=>{
      const range=/^bytes=(\d+)-(\d*)$/.exec(route.request().headers().range ?? '');
      const start=range ? Number(range[1]) : 0,end=range?.[2] ? Math.min(Number(range[2]),bytes.length-1) : bytes.length-1;
      await route.fulfill({status:range ? 206 : 200,headers:{'content-type':'video/mp4','accept-ranges':'bytes',...(range ? {'content-range':`bytes ${start}-${end}/${bytes.length}`} : {})},body:bytes.subarray(start,end+1)});
    });
    await setup(page,'http://127.0.0.1:1420/recording-fixture.mp4');
    await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
    await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
    await page.evaluate(()=>{(window as unknown as {recordingFixture:{capture:{metadata:unknown}}}).recordingFixture.capture.metadata={width:128,height:64,durationSeconds:2,averageFrameRate:{numerator:24,denominator:1},nominalFrameRate:{numerator:24,denominator:1}};});
    await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
    const video=page.getByLabel('Исходное видео',{exact:true});
    await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
    expect(await video.evaluate(v=>[(v as HTMLVideoElement).videoWidth,(v as HTMLVideoElement).videoHeight])).toEqual([128,64]);
    await page.getByLabel('Обрезать готовую запись').check();
    await page.getByLabel('Позиция в исходном видео').fill('0.5');
    await expect.poll(()=>video.evaluate(v=>(v as HTMLVideoElement).currentTime)).toBeCloseTo(0.5,2);
    await expect(page.getByRole('button',{name:'Начало по текущему кадру',exact:true})).toBeEnabled();
    await page.getByRole('button',{name:'Начало по текущему кадру',exact:true}).click();
    await expect(page.getByLabel('Начало записи, с',{exact:true})).toHaveValue('0.5');
    await page.getByLabel('Ширина области, %',{exact:true}).fill('50');
    await page.getByLabel('Область X, %',{exact:true}).fill('50');
    const canvas=page.getByLabel('Кадр после обрезки',{exact:true});
    const pixels=()=>canvas.evaluate(c=>Array.from((c as HTMLCanvasElement).getContext('2d')!.getImageData(240,240,1,1).data));
    await expect.poll(async()=>(await pixels())[2]).toBeGreaterThan(200);
    expect((await pixels())[0]).toBeLessThan(30);
    await page.getByRole('button',{name:'Добавить фрагмент записи',exact:true}).click();
    await page.getByLabel('Ширина области, %',{exact:true}).fill('50');
    await expect.poll(async()=>(await pixels())[0]).toBeGreaterThan(200);
    expect((await pixels())[2]).toBeLessThan(30);
    await page.getByLabel('Фрагмент записи',{exact:true}).selectOption({label:'Фрагмент 1'});
    await expect.poll(async()=>(await pixels())[2]).toBeGreaterThan(200);
    await video.evaluate(async v=>{Object.assign(window,{retainedRecordingPlayer:v});await (v as HTMLVideoElement).play();});
    await page.getByRole('button',{name:'Закрыть запись экрана',exact:true}).click();
    await expect.poll(()=>page.evaluate(()=>{const v=(window as unknown as {retainedRecordingPlayer:HTMLVideoElement}).retainedRecordingPlayer;return [v.paused,v.getAttribute('src')];})).toEqual([true,null]);
  }finally{rmSync(directory,{recursive:true,force:true});}
});

for(const failCancel of [false,true])test(`return from prepared review preserves source and allows changed preparation (cancel failure=${failCancel})`,async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await page.getByLabel('Название записи',{exact:true}).fill('Первый вариант');
  await page.getByRole('button',{name:'Подготовить запись для LCD',exact:true}).click();
  const firstId=await page.evaluate(()=>(window as unknown as {recordingFixture:{job:{id:string}}}).recordingFixture.job.id);
  if(failCancel)await page.evaluate(()=>{(window as unknown as {recordingFixture:{failCancel:boolean}}).recordingFixture.failCancel=true;});
  const back=page.getByRole('button',{name:'Вернуться к редактированию записи',exact:true});
  await back.click();
  if(failCancel){
    await expect(page.getByRole('region',{name:'Запись экрана',exact:true}).getByRole('alert')).toContainText('Повторите возврат');
    await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeDisabled();
    await back.click();
  }
  await expect(page.getByLabel('Название записи',{exact:true})).toHaveValue('Первый вариант');
  await expect(page.getByRole('heading',{name:'Запись экрана',exact:true})).toBeFocused();
  await page.getByLabel('Название записи',{exact:true}).fill('Второй вариант');
  await page.getByLabel('Размещение записи',{exact:true}).selectOption('cover');
  await page.getByRole('button',{name:'Подготовить запись для LCD',exact:true}).click();
  await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
  const f=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[];job:{id:string};edit:{name:string;fit:string};capture:{phase:string}}}).recordingFixture);
  expect(f.job.id).not.toBe(firstId);expect(f.edit.name).toBe('Второй вариант.mp4');expect(f.edit.fit).toBe('cover');
  expect(f.capture.phase).toBe('ready');expect(f.calls).not.toContain('screen_record_cancel');expect(f.calls).not.toContain('media_confirm');
});

for(const limited of [false,true])test(`operation choice whole recording prepares without publishing, limit=${limited}`,async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  if(limited)await page.evaluate(()=>{const f=(window as unknown as {recordingFixture:{capture:{phase:string;recordedMillis:number}}}).recordingFixture;f.capture.phase='limit_reached';f.capture.recordedMillis=180000;});
  else await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить запись целиком',exact:true}).click();
  await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
  const fixture=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[];edit:{cut:unknown;fit:string};capture:{phase:string}}}).recordingFixture);
  expect(fixture.capture.phase).toBe('ready');expect(fixture.edit.cut).toBeNull();expect(fixture.edit.fit).toBe('stretch');
  expect(fixture.calls.filter(c=>c==='screen_record_prepare')).toHaveLength(1);expect(fixture.calls).not.toContain('media_confirm');
});
test('manual operation cancellation retains paused capture and allows resume',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Что сделать с записью?'});await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true})).toBeFocused();
  await page.getByRole('button',{name:'Продолжить запись',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls);
  expect(calls).not.toContain('screen_record_prepare');expect(calls).not.toContain('media_confirm');
});

test('failed pause does not open operation chooser or finalize capture',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{failPause:boolean}}).recordingFixture.failPause=true;});
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('pause rejected');
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeEnabled();
});
for(const clear of [false,true])test(`deferred finish preserves import intent and clear cancels it, clear=${clear}`,async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{deferFinish:boolean}}).recordingFixture.deferFinish=true;});
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить запись целиком',exact:true}).click();
  await expect(page.getByRole('status').filter({hasText:'Завершение записи'})).toBeVisible();
  if(clear){await page.getByRole('button',{name:'Очистить запись…',exact:true}).click();await page.getByRole('button',{name:'Подтвердить очистку записи',exact:true}).click();await expect(page.getByRole('button',{name:'Выбрать экран или окно',exact:true})).toBeEnabled();}
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{capture:{phase:string}}}).recordingFixture.capture.phase='ready';});
  if(!clear)await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
  else {await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();}
  const calls=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls);
  expect(calls.filter(c=>c==='screen_record_prepare')).toHaveLength(clear?0:1);expect(calls).not.toContain('media_confirm');
});

test('late recording status cannot dismiss paused operation dialog',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{holdStatus:boolean}}).recordingFixture.holdStatus=true;});
  await expect.poll(()=>page.evaluate(()=>!!(window as unknown as {recordingFixture:{releaseStatus:unknown}}).recordingFixture.releaseStatus)).toBe(true);
  await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Что сделать с записью?'});await expect(dialog).toBeVisible();
  await page.evaluate(async()=>{(window as unknown as {recordingFixture:{releaseStatus:()=>void}}).recordingFixture.releaseStatus();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));});
  await expect(dialog).toBeVisible();
  await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
  await expect(page.getByLabel('Исходное видео',{exact:true})).toBeVisible();
});

for(const failure of ['none','reject','lost'])test(`paused capture region validates, applies and reads back (${failure})`,async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  await page.getByLabel('Захват X, %',{exact:true}).fill('50');
  const apply=page.getByRole('button',{name:'Применить область захвата',exact:true});
  const resume=page.getByRole('button',{name:'Продолжить запись',exact:true});
  await expect(apply).toBeDisabled();await expect(resume).toBeDisabled();
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('50');
  await page.evaluate(failure=>{const f=(window as unknown as {recordingFixture:{failRegion:boolean;lostRegion:boolean}}).recordingFixture;f.failRegion=failure==='reject';f.lostRegion=failure==='lost';},failure);
  await apply.click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {recordingFixture:{regions:unknown[]}}).recordingFixture.regions)).toEqual([{x:0.5,y:0,width:0.5,height:1}]);
  if(failure==='reject'){
    await expect(page.getByRole('alert')).toContainText('region rejected');await expect(resume).toBeDisabled();
    await page.getByRole('button',{name:'Вернуть применённую область',exact:true}).click();
    await expect(page.getByLabel('Захват X, %',{exact:true})).toHaveValue('0');
  }else{
    await expect(page.getByLabel('Применённая область захвата')).toContainText('X 50.0%');
    if(failure==='lost')await expect(page.getByRole('alert')).toContainText('lost region reply');
  }
  await expect(resume).toBeEnabled();await resume.click();
  await expect(page.getByLabel('Захват X, %',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  await page.getByRole('button',{name:'Весь источник',exact:true}).click();
  if(failure!=='reject'){
    await page.evaluate(()=>{(window as unknown as {recordingFixture:{lostRegion:boolean}}).recordingFixture.lostRegion=false;});
    await apply.click();await expect(page.getByLabel('Применённая область захвата')).toContainText('ширина 100.0%');
  }
  await expect(resume).toBeEnabled();
});

test('restoring fractional applied region permits resume despite percentage rounding',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('97.9');
  await page.getByLabel('Захват X, %',{exact:true}).fill('2.1');
  const apply=page.getByRole('button',{name:'Применить область захвата',exact:true});
  await apply.click();await expect(apply).toBeDisabled();
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('90');
  await expect(page.getByRole('button',{name:'Продолжить запись',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Вернуть применённую область',exact:true}).click();
  await expect(apply).toBeDisabled();
  await page.getByRole('button',{name:'Продолжить запись',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
});

test('paused source preview selects crop with pointer and keyboard and releases image on resume',async({page})=>{
  await setup(page);await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  const selector=page.getByRole('button',{name:'Выбор области захвата на кадре',exact:true});await expect(selector).toBeVisible();
  const box=(await selector.locator('canvas').boundingBox())!;
  await page.mouse.move(box.x+box.width*.55,box.y+box.height*.1);await page.mouse.down();
  await page.mouse.move(box.x+box.width*.95,box.y+box.height*.9);await page.mouse.up();
  await selector.press('ArrowLeft');
  const x=await page.getByLabel('Захват X, %',{exact:true}).inputValue();expect(Number(x)).toBeCloseTo(54,0);
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('20');
  const preview=page.getByLabel('Предпросмотр области захвата',{exact:true});
  await expect.poll(()=>preview.evaluate((element:HTMLCanvasElement)=>Array.from(element.getContext('2d')!.getImageData(64,32,1,1).data))).toEqual([0,0,254,255]);
  await page.getByRole('button',{name:'Применить область захвата',exact:true}).click();
  await page.getByRole('button',{name:'Продолжить запись',exact:true}).click();
  await expect(selector).toHaveCount(0);
  const urls=await page.evaluate(()=>(window as unknown as {recordingFixture:{snapshotUrls:string[];revokedUrls:string[]}}).recordingFixture);
  expect(urls.snapshotUrls.length).toBeGreaterThan(0);for(const url of urls.snapshotUrls)expect(urls.revokedUrls).toContain(url);
});
test('failed source preview can retry without losing numeric region draft',async({page})=>{
  await setup(page);await page.evaluate(()=>{(window as unknown as {recordingFixture:{failSnapshot:boolean}}).recordingFixture.failSnapshot=true;});
  await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();await page.getByRole('button',{name:'Приостановить запись',exact:true}).click();
  await expect(page.getByText('Не удалось загрузить кадр источника. Повторите запрос или задайте область числами.')).toBeVisible();
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('50');
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{failSnapshot:boolean}}).recordingFixture.failSnapshot=false;});
  await page.getByRole('button',{name:'Обновить кадр источника',exact:true}).click();
  await expect(page.getByRole('button',{name:'Выбор области захвата на кадре',exact:true})).toBeVisible();
  await expect(page.getByLabel('Ширина захвата, %',{exact:true})).toHaveValue('50');
});
for (const phase of ['setup','paused'] as const) test(`late ${phase} snapshot cannot restore preview after start or resume`,async({page})=>{
  await setup(page);
  if(phase==='paused') {
    await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();
    await expect(page.getByRole('button',{name:'Выбор области захвата на кадре',exact:true})).toBeVisible();
    await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  }
  await page.evaluate(()=>{(window as unknown as {recordingFixture:{holdSnapshot:boolean}}).recordingFixture.holdSnapshot=true;});
  await page.getByRole('button',{name:phase==='setup'?'Выбрать экран или окно':'Приостановить запись',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>!!(window as unknown as {recordingFixture:{releaseSnapshot:unknown}}).recordingFixture.releaseSnapshot)).toBe(true);
  await page.getByRole('button',{name:phase==='setup'?'Начать запись экрана':'Продолжить запись',exact:true}).click();
  const before=await page.evaluate(()=>(window as unknown as {recordingFixture:{snapshotUrls:string[]}}).recordingFixture.snapshotUrls.length);
  await page.evaluate(async()=>{(window as unknown as {recordingFixture:{releaseSnapshot:()=>void}}).recordingFixture.releaseSnapshot();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));});
  await expect(page.getByRole('region',{name:'Кадр для выбора области захвата',exact:true})).toHaveCount(0);
  expect(await page.evaluate(()=>(window as unknown as {recordingFixture:{snapshotUrls:string[]}}).recordingFixture.snapshotUrls.length)).toBe(before);
  expect(await page.evaluate(()=>{const f=(window as unknown as {recordingFixture:{snapshotUrls:string[];revokedUrls:string[]}}).recordingFixture;return f.snapshotUrls.every(url=>f.revokedUrls.includes(url));})).toBe(true);
});

test('initial region setup requires explicit apply and start and permits cancellation',async({page})=>{
  await setup(page);
  await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();
  await expect(page.getByText('Выберите область перед записью · 0.0 с',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Выбор области захвата на кадре',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true})).toHaveCount(0);
  await page.getByLabel('Захват X, %',{exact:true}).fill('50');
  await page.getByLabel('Ширина захвата, %',{exact:true}).fill('50');
  await expect(page.getByRole('button',{name:'Начать запись экрана',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Применить область захвата',exact:true}).click();
  await expect(page.getByRole('button',{name:'Начать запись экрана',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
  await expect(page.getByRole('button',{name:'Приостановить запись',exact:true})).toBeVisible();
  await expect(page.getByLabel('Применённая область захвата',{exact:true})).toContainText('X 50.0%');
  await page.getByRole('button',{name:'Очистить запись…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить очистку записи',exact:true}).click();
  await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();
  await page.getByRole('button',{name:'Закрыть запись экрана',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {recordingFixture:{capture:{phase:string}}}).recordingFixture.capture.phase)).toBe('cancelled');
});

for (const fault of ['failAdmit', 'failPrepare'] as const) {
  test(`failed recording ${fault} returns to editing and can prepare again`, async ({page}) => {
    await setup(page);
    await page.getByRole('button',{name:'Выбрать экран или окно',exact:true}).click();
    await page.getByRole('button',{name:'Начать запись экрана',exact:true}).click();
    await page.getByRole('button',{name:'Завершить и просмотреть запись',exact:true}).click();
    await page.getByRole('button',{name:'Редактировать запись',exact:true}).click();
    await page.getByLabel('Название записи',{exact:true}).fill('Сохранённый черновик');
    await page.evaluate(fault => {
      const f=(window as unknown as {recordingFixture:{failAdmit:boolean;failPrepare:boolean}}).recordingFixture;
      f[fault]=true;
    },fault);
    await page.getByRole('button',{name:'Подготовить запись для LCD',exact:true}).click();
    await expect(page.getByRole('alert').filter({hasText:/rejected/})).toBeVisible();
    await page.getByRole('button',{name:'Вернуться к редактированию записи',exact:true}).click();
    await expect(page.getByLabel('Название записи',{exact:true})).toHaveValue('Сохранённый черновик');
    await expect(page.getByRole('button',{name:'Подготовить запись для LCD',exact:true})).toBeEnabled();
    await page.evaluate(() => {
      const f=(window as unknown as {recordingFixture:{failAdmit:boolean;failPrepare:boolean;calls:string[];capture:{phase:string}}}).recordingFixture;
      f.failAdmit=false;f.failPrepare=false;
      if(f.capture.phase!=='ready'||f.calls.includes('screen_record_cancel')||f.calls.includes('media_confirm'))throw new Error('capture was lost or published');
    });
    await page.getByRole('button',{name:'Подготовить запись для LCD',exact:true}).click();
    await expect(page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true})).toBeEnabled();
    await page.getByRole('button',{name:'Сохранить запись в библиотеку',exact:true}).click();
    await expect(page.getByText('Запись сохранена в библиотеке.',{exact:true})).toBeVisible();
  });
}

test('English recording starts with a localized draft name and keeps the user name for preparation',async({page})=>{
 await setup(page,undefined,'en-US');
 await page.getByRole('button',{name:'Select screen or window',exact:true}).click();
 await page.getByRole('button',{name:'Start screen recording',exact:true}).click();
 await page.getByRole('button',{name:'Finish and review recording',exact:true}).click();
 await page.getByRole('button',{name:'Edit recording',exact:true}).click();
 await expect(page.getByLabel('Recording name',{exact:true})).toHaveValue('Screen recording');
 await page.getByLabel('Recording name',{exact:true}).fill('Запись экрана');
 await page.getByRole('button',{name:'Prepare recording for LCD',exact:true}).click();
 const fixture=await page.evaluate(()=>(window as unknown as {recordingFixture:{edit:{name:string};calls:string[]}}).recordingFixture);
 expect(fixture.edit.name).toBe('Запись экрана.mp4');expect(fixture.calls).not.toContain('media_confirm');
});


for(const language of ['en-US','ru-RU'])test(`worker failure can be cleared and a new capture started (${language})`,async({page})=>{
  await setup(page,undefined,language);
  const english=language==='en-US';
  const choose=english?'Select screen or window':'Выбрать экран или окно';
  await page.getByRole('button',{name:choose,exact:true}).click();
  await expect(page.getByRole('button',{name:english?'Start screen recording':'Начать запись экрана',exact:true})).toBeVisible();
  await page.evaluate(()=>{
    const fixture=(window as unknown as {recordingFixture:{capture:{phase:string;message:string}}}).recordingFixture;
    fixture.capture.phase='failed';fixture.capture.message='Recording worker response timed out';
  });
  await expect(page.getByRole('alert')).toHaveText(english?'The recording process is not responding. Recording stopped. Select the source again.':'Процесс записи не отвечает. Запись остановлена. Выберите источник заново.');
  await page.getByRole('button',{name:english?'Clear recording…':'Очистить запись…',exact:true}).click();
  await page.getByRole('button',{name:english?'Confirm clearing the recording':'Подтвердить очистку записи',exact:true}).click();
  await page.getByRole('button',{name:choose,exact:true}).click();
  await expect(page.getByRole('button',{name:english?'Start screen recording':'Начать запись экрана',exact:true})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {recordingFixture:{calls:string[]}}).recordingFixture.calls);
  expect(calls.filter(method=>method==='screen_record_cancel').length).toBeGreaterThan(0);
  expect(calls).not.toContain('media_confirm');
});
