import {test,expect,type Page} from '@playwright/test';
import {setup,type Fixture} from './fixtures/group-archive';
const copy={
 'en-US':{panel:'Settings group backup',settings:'Settings',group:'Settings group',choose:'Choose Windows ZIP…',record:'Windows source controller',prepare:'Review Windows settings',apply:'Import and apply',retry:'Retry import with the same ID',recover:'Read group result',roles:['CPU · Load (%)','CPU · Temperature (°C)','GPU · Load (%)','GPU · Temperature (°C)','Pump (RPM)','Coolant temperature (°C)']},
 'ru-RU':{panel:'Резервная копия группы настроек',settings:'Настройки',group:'Группа настроек',choose:'Выбрать Windows ZIP…',record:'Исходный контроллер Windows',prepare:'Проверить настройки Windows',apply:'Импортировать и применить',retry:'Повторить импорт с тем же ID',recover:'Прочитать результат группы',roles:['CPU · Загрузка (%)','CPU · Температура (°C)','GPU · Загрузка (%)','GPU · Температура (°C)','Помпа (RPM)','Температура жидкости (°C)']}
};
type Language=keyof typeof copy;
const panel=(p:Page,l:Language)=>p.locator('details').filter({has:p.locator('summary').filter({hasText:copy[l].panel})}).first();
async function begin(p:Page,l:Language) {await setup(p,l);await p.getByRole('combobox',{name:copy[l].group,exact:true}).selectOption('fan_and_lighting');await p.getByRole('button',{name:copy[l].choose,exact:true}).click();}
async function bindings(p:Page,l:Language) {await p.getByRole('combobox',{name:copy[l].record,exact:true}).selectOption('1');for(const [i,label] of copy[l].roles.entries())await p.getByLabel(label,{exact:true}).fill(i===1?'cpu:test':`explicit:role:${i}`);}
async function calls(p:Page) {return p.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls);}
for(const language of ['en-US','ru-RU'] as const) {
 const c=copy[language];
 test(`${language}: explicit bindings and read-only preview fence Apply until prepared again`,async({page})=>{
  await begin(page,language);const area=panel(page,language);await expect(area.getByRole('button',{name:c.prepare,exact:true})).toBeDisabled();await bindings(page,language);
  await page.getByLabel(c.roles[0],{exact:true}).fill('cpu:test');await expect(area.getByRole('button',{name:c.prepare,exact:true})).toBeDisabled();
  await page.getByLabel(c.roles[0],{exact:true}).fill('explicit:load');await area.getByRole('button',{name:c.prepare,exact:true}).click();await expect(area.getByRole('button',{name:c.apply,exact:true})).toBeEnabled();expect((await calls(page)).filter(v=>v.args.action==='finish')).toHaveLength(0);
  await page.getByLabel(c.roles[0],{exact:true}).fill('newer:load');await expect(area.getByRole('button',{name:c.apply,exact:true})).toBeDisabled();await expect(area.getByRole('status').filter({hasText:language==='en-US'?'Bindings changed':'Привязки изменились'})).toBeVisible();
  await area.getByRole('button',{name:c.prepare,exact:true}).click();await expect(area.getByRole('button',{name:c.apply,exact:true})).toBeEnabled();
  await area.locator('.windows-lighting-review > summary').click();await expect(area.locator('.windows-lighting-review .mode')).toHaveCount(16);
  for(const [width,height] of [[1280,1000],[1024,900]]) {await page.setViewportSize({width,height});await area.locator('.windows-bindings').scrollIntoViewIfNeeded();expect(await area.evaluate(node=>node.scrollWidth<=node.clientWidth)).toBe(true);await page.screenshot({path:`../.impeccable/review/windows-control-bindings-${language}-${width}.png`});}
  await area.getByRole('button',{name:c.apply,exact:true}).click();await expect(area.getByRole('status').filter({hasText:language==='en-US'?'Group save confirmed':'Сохранение группы подтверждено'})).toBeVisible();expect(await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.commits)).toBe(1);
 });
 test(`${language}: rejected preparation preserves IDs and cannot enable Apply`,async({page})=>{
  await begin(page,language);await bindings(page,language);const area=panel(page,language);await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.rejectWindows=true);await area.getByRole('button',{name:c.prepare,exact:true}).click();await expect(area.getByRole('alert')).toContainText(language==='en-US'?'Windows archive is not prepared':'Windows-архив не подготовлен');await expect(page.getByLabel(c.roles[1],{exact:true})).toHaveValue('cpu:test');await expect(area.getByRole('button',{name:c.apply,exact:true})).toHaveCount(0);expect((await calls(page)).filter(v=>v.args.action==='finish')).toHaveLength(0);
 });
 test(`${language}: uncertain result explicitly retries the original operation; restart only reads receipt`,async({page})=>{
  await begin(page,language);await bindings(page,language);const area=panel(page,language);await area.getByRole('button',{name:c.prepare,exact:true}).click();await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.lostFinish=true);await area.getByRole('button',{name:c.apply,exact:true}).click();await expect(area.getByRole('alert')).toContainText(language==='en-US'?'Windows import result is unconfirmed':'Результат Windows импорта не подтверждён');await area.getByRole('button',{name:c.retry,exact:true}).click();
  const writes=(await calls(page)).filter(v=>v.args.action==='finish');expect(writes).toHaveLength(2);expect(writes[1].args.request).toEqual(writes[0].args.request);expect(writes[1].args).not.toHaveProperty('options');expect(await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.commits)).toBe(1);
  const pending=await page.evaluate(()=>JSON.parse(localStorage.getItem('lconnect.group.pending')!));await page.reload();await page.getByRole('button',{name:c.settings,exact:true}).click();await page.getByText(c.panel,{exact:true}).click();await area.getByRole('button',{name:c.recover,exact:true}).click();expect((await calls(page)).filter(v=>v.args.action==='finish')).toHaveLength(0);expect((await calls(page)).find(v=>v.args.action==='recover')?.args.request).toEqual(pending);await expect(area.locator('.windows-bindings')).toHaveCount(0);
 });
 test(`${language}: another selected device cannot prepare or apply this source`,async({page})=>{
  await begin(page,language);await bindings(page,language);const area=panel(page,language);await area.getByRole('button',{name:c.prepare,exact:true}).click();await page.getByRole('combobox',{name:language==='en-US'?'Device':'Устройство',exact:true}).selectOption('other');await expect(area.getByRole('button',{name:c.prepare,exact:true})).toBeDisabled();await expect(area.getByRole('button',{name:c.apply,exact:true})).toBeDisabled();expect((await calls(page)).filter(v=>v.args.action==='finish')).toHaveLength(0);
 });
}
