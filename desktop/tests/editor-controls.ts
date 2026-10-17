import {expect,type Page} from '@playwright/test';

export async function openEditorSection(page:Page,name:string) {
  await expect(page.getByText(/^(Daemon connected|Daemon подключён)$/,{exact:true})).toBeVisible();
  const summary=page.locator('summary').filter({hasText:new RegExp(`^${name}$`)});
  if(!await summary.evaluate(node=>(node.parentElement as HTMLDetailsElement).open))await summary.click();
}
export async function loadControls(page:Page){await openEditorSection(page,'Загрузить или остановить|Load or stop');}
export async function addControls(page:Page){await openEditorSection(page,'Добавить слой|Add layer');}
export async function selectLayer(page:Page,index:number){await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator(`.layer-row[data-layer-index="${index}"]`).click();await expect(page.locator(`.inspector[data-layer-index="${index}"]`)).toBeVisible();}
