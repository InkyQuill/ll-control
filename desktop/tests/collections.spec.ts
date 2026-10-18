import { test, expect } from '@playwright/test';

test('collections migrate, remove and restore membership without deleting assets; reopening waits for pending save', async ({ page }) => {
  await page.addInitScript(() => {
    type Collection = {version:number;revision:number;images:string[];backgrounds:{kind:string;key:string}[];watermarks:string[]};
    let collections: Collection | null = null;
    let lose = false, failRead = false;
    let hold = false, release: (() => void) | undefined;
    const writes: string[] = [];
    Object.assign(window, { loseCollectionReply: () => {lose=true;}, recoverCollectionRead: () => {failRead=false;}, holdCollectionWrite: () => { hold = true; }, releaseCollectionWrite: () => release?.(), collectionWrites:writes,
      __TAURI_INTERNALS__: { invoke: async (_: string, request: {method:string;params:{collections:Collection;expected_revision:number}}) => {
        if (_ === 'desktop_set_language') return null;
        switch(request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
          case 'list_devices': return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
          case 'list_sensors': return [];
          case 'get_lcd_state': return {source:'Idle',last_error:null,requested:{brightness_percent:50,rotation_degrees:0,mode:'LocalUI'}};
          case 'get_screen_collections': if (failRead) throw {message:'Disconnected'}; return {collections:structuredClone(collections),persistence:{pending:false,saved:!!collections,last_error:null}};
          case 'list_named_theme_assets': return [{key:'user',name:'Мой рисунок'},{key:'builtin-orbit-mark-v1',name:'Орбита',is_default:true},{key:'builtin-midnight-v1',name:'Полночь',is_default:true,contexts:['image','background']}];
          case 'list_prepared_clips': return [{key:'bcf5a102bf2a426286d34310cad4a001',name:'Орбита видео',is_default:true}];
          case 'migrate_screen_collections':
            writes.push(request.method);
            collections={version:1,revision:1,images:['builtin-orbit-mark-v1','builtin-midnight-v1','user'],backgrounds:[{kind:'image',key:'builtin-orbit-mark-v1'},{kind:'video',key:'bcf5a102bf2a426286d34310cad4a001'}],watermarks:['builtin-orbit-mark-v1','user']}; return structuredClone(collections);
          case 'set_screen_collections':
            writes.push(request.method);
            if (hold) { hold=false; await new Promise<void>(resolve => {release=resolve;}); }
            if (collections?.revision !== request.params.expected_revision) throw {message:'stale revision'};
            collections=structuredClone(request.params.collections); if (lose) {lose=false;failRead=true;throw {message:'Reply lost after commit'};} return structuredClone(collections);
          default: return [];
        }
      }}
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Коллекции LCD',exact:true}).click();
  await page.getByRole('button',{name:'Перенести каталог в коллекции'}).click();
  const remove = page.getByRole('button',{name:'Убрать из коллекции: Мой рисунок'});
  await expect(remove).toBeVisible();
  await expect(page.getByRole('button',{name:'Убрать из коллекции: Орбита',exact:true})).toBeDisabled();
  await remove.click();
  await expect(page.getByRole('button',{name:'Обновить коллекции'})).toBeFocused();
  await page.evaluate(() => (window as unknown as {holdCollectionWrite:()=>void}).holdCollectionWrite());
  await page.getByRole('button',{name:'Сохранить коллекции'}).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Коллекции LCD',exact:true}).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Коллекции LCD',exact:true}).click();
  await expect(remove).toBeVisible();
  await expect(remove).toBeDisabled();
  await page.evaluate(() => (window as unknown as {releaseCollectionWrite:()=>void}).releaseCollectionWrite());
  await expect(remove).toHaveCount(0);
  await page.getByLabel('Добавить из каталога').selectOption('image:user');
  await page.getByRole('button',{name:'Добавить в список'}).click();
  await page.getByRole('button',{name:'Сохранить коллекции'}).click();
  await expect(remove).toBeEnabled();
  await remove.click();
  await page.evaluate(() => (window as unknown as {loseCollectionReply:()=>void}).loseCollectionReply());
  await page.getByRole('button',{name:'Сохранить коллекции'}).click();
  await expect(page.getByText(/Не удалось проверить коллекции/)).toBeVisible();
  await expect(page.getByRole('button',{name:'Сохранить коллекции'})).toBeDisabled();
  await expect(page.getByLabel('Добавить из каталога')).toBeDisabled();
  await page.evaluate(() => (window as unknown as {recoverCollectionRead:()=>void}).recoverCollectionRead());
  await page.getByRole('button',{name:'Обновить коллекции'}).click();
  await expect(remove).toHaveCount(0);
  await expect(page.getByLabel('Добавить из каталога')).toBeEnabled();
  expect(await page.evaluate(() => (window as unknown as {collectionWrites:string[]}).collectionWrites)).toEqual(['migrate_screen_collections','set_screen_collections','set_screen_collections','set_screen_collections']);
});
