import {test,expect,type Page} from '@playwright/test';
async function setup(page:Page){
 await page.addInitScript(()=>{
  const text=(value:string)=>({kind:'text',value});
  const field=(key:string,value:unknown)=>({key,value,source:'Fixture sysfs',unavailable_reason:value===null?'Not accessible':null});
  const f={fail:false,offline:false,calls:[] as Record<string,unknown>[],spec:{version:1,collected_at_unix_ms:1_800_000_000_000,groups:[
   {kind:'cpu',items:[{id:'cpu:0',title:'Fixture CPU',fields:[field('logical_threads',{kind:'count',value:16})]}],unavailable_reason:null},
   {kind:'gpu',items:[{id:'pci:0',title:'Fixture GPU A',fields:[field('model',text('Fixture GPU A'))]},{id:'pci:1',title:'Fixture GPU B',fields:[field('driver',null)]}],unavailable_reason:null},
   {kind:'memory',items:[{id:'memory:os',title:'OS-visible memory',fields:[field('os_visible_capacity',{kind:'bytes',value:34359738368}),field('reported_modules',null)]}],unavailable_reason:null},
   {kind:'monitors',items:[],unavailable_reason:'No monitors'},
   {kind:'motherboard',items:[{id:'board:system',title:'Fixture Board',fields:[field('model',text('Fixture Board'))]}],unavailable_reason:null},
   {kind:'storage',items:[],unavailable_reason:'Not accessible'}
  ]}};
  Object.assign(window,{specFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   if(command==='group_archive_status')return null;
   if(args.method==='get_system_spec'){f.calls.push(structuredClone(args.params as Record<string,unknown>));if(f.fail)throw {message:'Fixture denied'};return structuredClone(f.spec);}
   if(f.offline)throw {message:'Disconnected fixture'};
   switch(args.method){case 'list_devices':case 'list_sensors':case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];default:return null;}
  }}});
 });
 await page.goto('/');await expect(page.getByText('Daemon подключён',{exact:true})).toBeVisible({timeout:15000});
 await page.getByRole('button',{name:'Сведения о системе',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Fixture CPU'})).toBeVisible();
}
test('static categories and per-component details work with no Lian Li device; missing values remain unavailable',async({page})=>{
 await setup(page);const section=page.getByRole('region',{name:'Сведения о системе',exact:true});
 await expect(section.getByText('16',{exact:true})).toBeVisible();
 await section.getByRole('button',{name:'GPU',exact:true}).click();await section.getByRole('combobox').selectOption('pci:1');
 await expect(section.getByRole('heading',{name:'Fixture GPU B'})).toBeVisible();await expect(section.getByText('Недоступно',{exact:true})).toBeVisible();
 await section.getByRole('button',{name:'Память',exact:true}).click();await expect(section.getByText('32 GiB',{exact:true})).toBeVisible();await expect(section.getByText(/отличается от суммы установленных/)).toBeVisible();
 for(const label of ['Мониторы','Накопители']){await section.getByRole('button',{name:label,exact:true}).click();await expect(section.getByRole('status')).toHaveText('Сведения в этой категории недоступны.');}
 await section.getByRole('button',{name:'Материнская плата',exact:true}).click();await expect(section.getByRole('heading',{name:'Fixture Board'})).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {specFixture:{calls:unknown[]}}).specFixture.calls)).toEqual([{refresh:false}]);
});
test('explicit refresh reports failure with previous data retained, then retries and replaces details',async({page})=>{
 await setup(page);const section=page.getByRole('region',{name:'Сведения о системе',exact:true});
 await page.evaluate(()=>{(window as unknown as {specFixture:{fail:boolean}}).specFixture.fail=true;});
 await section.getByRole('button',{name:'Обновить сведения'}).click();await expect(section.getByRole('alert')).toContainText('Fixture denied');await expect(section.getByRole('heading',{name:'Fixture CPU'})).toBeVisible();await expect(section.getByText(/актуальность не подтверждена/)).toBeVisible();
 await page.evaluate(()=>{const f=(window as unknown as {specFixture:{fail:boolean;spec:{groups:{items:{title:string}[]}[]}}}).specFixture;f.fail=false;f.spec.groups[0].items[0].title='Refreshed CPU';});
 await section.getByRole('button',{name:'Обновить сведения'}).click();await expect(section.getByRole('heading',{name:'Refreshed CPU'})).toBeVisible();await expect(section.getByRole('alert')).toHaveCount(0);
 expect(await page.evaluate(()=>(window as unknown as {specFixture:{calls:unknown[]}}).specFixture.calls)).toEqual([{refresh:false},{refresh:true},{refresh:true}]);
});
test('daemon loss disables refresh and marks retained data stale',async({page})=>{
 await setup(page);const section=page.getByRole('region',{name:'Сведения о системе',exact:true});
 await page.evaluate(()=>{(window as unknown as {specFixture:{offline:boolean}}).specFixture.offline=true;});
 await expect(section.getByText(/Нет подключения к daemon/)).toBeVisible();await expect(section.getByRole('button',{name:'Обновить сведения'})).toBeDisabled();await expect(section.getByRole('heading',{name:'Fixture CPU'})).toBeVisible();
});
