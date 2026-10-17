import {test,expect,type Page} from '@playwright/test';
async function setup(page:Page,language:'en-US'|'ru-RU'='ru-RU'){
 await page.addInitScript(language=>{
  localStorage.setItem('lconnect.interface-language',language);
  const text=(value:string)=>({kind:'text',value});
  const field=(key:string,value:unknown)=>({key,value,source:'Fixture sysfs',unavailable_reason:value===null?'Not accessible':null});
  const f={language,fail:false,offline:false,invalid:false,diagnostic:'Fixture denied',calls:[] as Record<string,unknown>[],spec:{version:1,collected_at_unix_ms:1_800_000_000_000,groups:[
   {kind:'cpu',items:[{id:'cpu:0',title:'Fixture CPU',fields:[field('logical_threads',{kind:'count',value:16})]}],unavailable_reason:null},
   {kind:'gpu',items:[{id:'pci:0',title:'Fixture GPU A',fields:[field('model',text('Fixture GPU A'))]},{id:'pci:1',title:'Fixture GPU B',fields:[field('driver',null)]}],unavailable_reason:null},
   {kind:'memory',items:[{id:'memory:os',title:'OS-visible memory',fields:[field('os_visible_capacity',{kind:'bytes',value:34359738368}),field('reported_modules',null)]}],unavailable_reason:null},
   {kind:'monitors',items:[],unavailable_reason:'No monitors'},
   {kind:'motherboard',items:[{id:'board:system',title:'Fixture Board',fields:[field('model',text('Fixture Board'))]}],unavailable_reason:null},
   {kind:'storage',items:[],unavailable_reason:'Not accessible'}
  ]}};
  Object.assign(window,{specFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
   if(command==='group_archive_status')return null;
   if(args.method==='get_system_spec'){f.calls.push(structuredClone(args.params as Record<string,unknown>));if(f.fail)throw {message:f.diagnostic};return f.invalid?{...structuredClone(f.spec),groups:[]}:structuredClone(f.spec);}
   if(f.offline)throw {message:'Disconnected fixture'};
   switch(args.method){case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:f.language};case 'list_devices':case 'list_sensors':case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];default:return null;}
  }}});
 },language);
 await page.goto('/?view=display');await expect(page.getByText(language==='en-US'?'Daemon connected':'Daemon подключён',{exact:true})).toBeVisible({timeout:15000});
 await page.getByRole('button',{name:language==='en-US'?'System information':'Сведения о системе',exact:true}).click();
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

async function specLanguage(page:Page,language:string) {
 await page.evaluate(language=>{(window as unknown as {specFixture:{language:string}}).specFixture.language=language;},language);
 await expect(page.getByRole('region',{name:language==='en-US'?'System information':'Сведения о системе',exact:true})).toBeVisible();
}
for(const language of ['en-US','ru-RU'] as const) {
 test(`Spec numbers and collection time follow language without refetch (${language})`,async({page})=>{
  await setup(page,language);const en=language==='en-US';
  await page.evaluate(()=>{
   const f=(window as unknown as {specFixture:{spec:{groups:{items:{fields:{key:string;value:unknown;source:string}[]}[]}[]}}}).specFixture;
   f.spec.groups[0].items[0].fields=[{key:'logical_threads',value:{kind:'count',value:16384},source:'Fixture sysfs'},{key:'frequency',value:{kind:'hertz',value:3200123},source:'Fixture sysfs'},{key:'capacity',value:{kind:'bytes',value:1610612736},source:'Fixture sysfs'}];
  });
  const refresh=()=>page.getByRole('button',{name:en?'Refresh information':'Обновить сведения',exact:true});
  await refresh().click();
  const verify=async(locale:string)=>{
   const expected=await page.evaluate(locale=>({count:(16384).toLocaleString(locale),hertz:(3200123).toLocaleString(locale)+' Hz',bytes:(1.5).toLocaleString(locale,{maximumFractionDigits:2})+' GiB',date:new Date(1800000000000).toLocaleString(locale)}),locale);
   const section=page.locator('section').filter({has:page.getByRole('heading',{name:'Fixture CPU',exact:true})});
   for(const value of [expected.count,expected.hertz,expected.bytes])await expect(section.locator('dd span').filter({hasText:value})).toBeVisible();
   await expect(section.locator('.updated')).toContainText(expected.date);
  };
  await verify(language);await specLanguage(page,en?'ru-RU':'en-US');await verify(en?'ru-RU':'en-US');
  expect(await page.evaluate(()=>(window as unknown as {specFixture:{calls:unknown[]}}).specFixture.calls)).toEqual([{refresh:false},{refresh:true}]);
 });
 test(`Spec backend diagnostic stays raw across language changes (${language})`,async({page})=>{
  await setup(page,language);const en=language==='en-US';
  await page.evaluate(()=>{Object.assign((window as unknown as {specFixture:object}).specFixture,{fail:true,diagnostic:'Настройки'});});
  await page.getByRole('button',{name:en?'Refresh information':'Обновить сведения',exact:true}).click();
  const expected=(english:boolean)=>english?'Could not read system information: Настройки. Retry the refresh.':'Не удалось получить сведения: Настройки. Повторите обновление.';
  await expect(page.getByRole('alert')).toHaveText(expected(en));
  await specLanguage(page,en?'ru-RU':'en-US');await expect(page.getByRole('alert')).toHaveText(expected(!en));
  await expect(page.getByRole('heading',{name:'Fixture CPU',exact:true})).toBeVisible();
  expect(await page.evaluate(()=>(window as unknown as {specFixture:{calls:unknown[]}}).specFixture.calls)).toHaveLength(2);
 });
 test(`Spec invalid response remains a localized owned failure (${language})`,async({page})=>{
  await setup(page,language);const en=language==='en-US';
  await page.evaluate(()=>{(window as unknown as {specFixture:{invalid:boolean}}).specFixture.invalid=true;});
  await page.getByRole('button',{name:en?'Refresh information':'Обновить сведения',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText(en?'Unsupported system information response':'Неподдерживаемый ответ сведений о системе');
  await specLanguage(page,en?'ru-RU':'en-US');
  await expect(page.getByRole('alert')).toContainText(en?'Неподдерживаемый ответ сведений о системе':'Unsupported system information response');
 });
 test(`Spec unknown field keys and hardware strings are data (${language})`,async({page})=>{
  await setup(page,language);const en=language==='en-US';
  await page.evaluate(()=>{
   const f=(window as unknown as {specFixture:{spec:{groups:{items:{fields:unknown[]}[]}[]}}}).specFixture;
   f.spec.groups[0].items[0].fields=['Настройки','constructor','__proto__'].map(key=>({key,value:{kind:'text',value:'Настройки'},source:'Настройки'}));
  });
  await page.getByRole('button',{name:en?'Refresh information':'Обновить сведения',exact:true}).click();
  const section=page.getByRole('region',{name:en?'System information':'Сведения о системе',exact:true});
  await expect(section.locator('dt')).toHaveText(['Настройки','constructor','__proto__']);
  await expect(section.locator('dd span')).toHaveText(['Настройки','Настройки','Настройки']);
  await expect(section.locator('dd small')).toHaveText(Array(3).fill((en?'Source: ':'Источник: ')+'Настройки'));
 });
}
