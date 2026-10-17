import { test, expect, type Page } from '@playwright/test';

async function setup(page:Page,rotation=0) {
  await page.addInitScript(({rotation})=>{
    const style={x:120,y:100,size:24,color:[255,255,255],align:'left',font:'mono',line_box:true,max_width:150,line_height:30,opacity:80};
    let theme={version:1,name:'Canvas test',background:[0,0,0],layers:[
      {kind:'image',asset:'logo',x:30,y:30,width:80,height:80,fit:'stretch',opacity:255},
      {kind:'sensor',sensor_id:'CPU',label:'My CPU',unit:'celsius',temperature_unit:'system',decimals:1,style:{...style,y:180},parts:{label:{...style,x:10,y:140},unit:{...style,x:250,y:210}}},
      {kind:'clock',twenty_four_hour:true,show_seconds:false,style:{...style,y:320}},
      {kind:'text',text:'Top layer',style:{...style}}
    ]};
    const writes:unknown[]=[];
    Object.assign(window,{canvasReplaceTheme:()=>{theme={...theme,layers:theme.layers.slice(0,1)};},canvasWrites:writes,__TAURI_INTERNALS__:{invoke:async(command:string,req:{method:string;params:Record<string,unknown>})=>{
      if(command==='desktop_set_language')return null;
      if(command==='import_image')return 'imported-image';
      switch(req.method){
        case 'list_devices':return [{id:'test',display_name:'LCD',family:'galahad2-lcdv2'}];
        case 'list_sensors':return [{id:'CPU',label:'CPU',unit:'celsius',value:40,status:'valid'},{id:'GPU',label:'GPU fan',unit:'rpm',value:1000,status:'valid'}];
        case 'get_lcd_state':return {source:'Idle',requested:{rotation_degrees:rotation},last_error:null};
        case 'get_lcd_theme':return theme;
        case 'list_profiles':return {revision:0,profiles:[]};
        case 'get_screen_collections':return {collections:null,persistence:{pending:false,last_error:null}};
        case 'preview_lcd_editor': {
          const draft=req.params.theme as typeof theme;
          const pixels=Array.from(Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0)));
          return {jpeg:pixels,background:pixels,layer_images:draft.layers.map((_,index)=>index===req.params.layer_index?pixels:[]),layers:draft.layers.map(layer=>layer.kind==='image'?{x:layer.x,y:layer.y,width:layer.width,height:layer.height}:layer.style?{x:layer.style.x,y:layer.style.y,width:120,height:layer.style.size}:null)};
        }
        case 'patch_lcd_config':writes.push(req.params);return {source:'Theme',requested:req.params.patch,last_error:null};
        case 'set_lcd_theme':writes.push(req.params);theme=req.params.theme as typeof theme;return {source:'Theme',requested:{rotation_degrees:rotation},last_error:null};
        default:throw new Error('Unexpected '+req.method);
      }
    }}});
  },{rotation});
  await page.goto('/?view=display');
  await page.getByText('Загрузить или остановить',{exact:true}).click();
  await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await page.getByText('Загрузить или остановить',{exact:true}).click();
  await expect(page.getByLabel('Название',{exact:true}).first()).toHaveValue('Canvas test');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();
  await ready(page);
}
async function ready(page:Page){await expect(page.getByText('Обновление холста…',{exact:true})).toHaveCount(0);}
const layer=(page:Page,index:number)=>page.locator(`.inspector[data-layer-index="${index}"]`);
async function drag(page:Page,name:string,dx:number,dy:number,cancel=false){
  const target=page.getByRole('button',{name,exact:true});
  await target.scrollIntoViewIfNeeded();
  const box=(await target.boundingBox())!;
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+dx,box.y+box.height/2+dy,{steps:5});
  if(cancel)await page.keyboard.press('Escape');
  await page.mouse.up();
  await ready(page);
}

test('pointer transforms, cancel, history and explicit Apply',async({page})=>{
  await setup(page);
  const square=(await page.getByRole('group',{name:'Холст LCD',exact:true}).boundingBox())!;
  const step=square.width/480;
  await drag(page,'Выбрать слой 4: Текст',40*step,20*step);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('160');
  await expect(layer(page,3).getByLabel('Y',{exact:true})).toHaveValue('120');
  await drag(page,'Выбрать слой 4: Текст',50*step,30*step,true);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('160');
  await page.getByRole('button',{name:'Отменить правку',exact:true}).click();await ready(page);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('120');
  await page.getByRole('button',{name:'Повторить правку',exact:true}).click();await ready(page);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('160');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();
  await drag(page,'Изменить размер выбранного слоя',40*step,40*step);
  await expect(layer(page,0).getByLabel('width',{exact:true})).toHaveValue('120');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
  await page.screenshot({path:test.info().outputPath('canvas.png')});
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'canvasWrites').length)).toBe(1);
});

test('keyboard moves, resizes and sensor replacement preserve imported styles',async({page})=>{
  await setup(page);
  const text=page.getByRole('button',{name:'Выбрать слой 4: Текст',exact:true});
  await text.focus();await page.keyboard.press('Control+ArrowRight');await ready(page);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('130');
  await page.keyboard.press('Shift+ArrowRight');await ready(page);
  await expect(layer(page,3).getByLabel('Размер',{exact:true})).toHaveValue('25');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="1"]').click();
  await layer(page,1).getByLabel('Источник датчика',{exact:true}).selectOption('GPU');await ready(page);
  await expect(layer(page,1).getByLabel('Подпись',{exact:true})).toHaveValue('My CPU');
  await expect(layer(page,1).getByRole('group',{name:'Подпись датчика',exact:true}).getByLabel('Непрозрачность текста, %',{exact:true})).toHaveValue('80');
  await layer(page,1).getByRole('group',{name:'Единица датчика',exact:true}).getByText('Дополнительные настройки текста',{exact:true}).click();
  await expect(layer(page,1).getByRole('group',{name:'Единица датчика',exact:true}).getByLabel('Ширина текста, px',{exact:true})).toHaveValue('150');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
});

test('rotated canvas drag maps to native coordinates',async({page})=>{
  await setup(page,90);
  const square=(await page.getByRole('group',{name:'Холст LCD',exact:true}).boundingBox())!;
  await drag(page,'Выбрать слой 4: Текст',square.width/12,0);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('120');
  await expect(layer(page,3).getByLabel('Y',{exact:true})).toHaveValue('60');
});

test('layer list reaches an obscured image and keyboard ordering preserves selection',async({page})=>{
  await setup(page);
  for(const [name,value] of [['x','130'],['y','102'],['width','60'],['height','20']]) {
    await layer(page,0).getByLabel(name,{exact:true}).fill(value);
  }
  await ready(page);
  const text=page.getByRole('button',{name:'Выбрать слой 4: Текст',exact:true});
  await text.click();
  await expect(page.locator('.layer-row[data-layer-index="3"]')).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();
  const handle=page.getByRole('button',{name:'Изменить размер выбранного слоя',exact:true});
  await handle.focus();await page.keyboard.press('Alt+ArrowUp');await ready(page);
  await expect(page.locator('.layer-row[data-layer-index="1"]')).toHaveAttribute('aria-pressed','true');
  await expect(layer(page,1).getByRole('heading',{name:'Изображение',exact:true})).toBeFocused();
  await expect(layer(page,1).getByLabel('width',{exact:true})).toHaveValue('60');
  await page.getByRole('button',{name:'Слои',exact:true}).click();
  await page.locator('.layer-row[data-layer-index="1"]').focus();await page.keyboard.press('Alt+ArrowUp');
  await expect(page.locator('.layer-row[data-layer-index="2"]')).toHaveAttribute('aria-pressed','true');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
});

test('sensor parts move together and losing pointer capture cancels the draft gesture',async({page})=>{
  await setup(page);
  const canvas=page.getByRole('group',{name:'Холст LCD',exact:true});
  const side=(await canvas.boundingBox())!.width;
  await drag(page,'Выбрать слой 2: Датчик',20*side/480,10*side/480);
  const label=layer(page,1).getByRole('group',{name:'Подпись датчика',exact:true});
  const unit=layer(page,1).getByRole('group',{name:'Единица датчика',exact:true});
  await expect(label.getByLabel('X',{exact:true})).toHaveValue('30');
  await expect(label.getByLabel('Y',{exact:true})).toHaveValue('150');
  await expect(unit.getByLabel('X',{exact:true})).toHaveValue('270');
  const target=page.getByRole('button',{name:'Выбрать слой 2: Датчик',exact:true});
  await target.scrollIntoViewIfNeeded();const box=(await target.boundingBox())!;
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+30,box.y+box.height/2+20,{steps:4});
  await canvas.evaluate(node=>{for(let id=0;id<10;id++)if(node.hasPointerCapture(id))node.releasePointerCapture(id);});
  await page.mouse.up();await ready(page);
  await expect(label.getByLabel('X',{exact:true})).toHaveValue('30');
  await expect(unit.getByLabel('X',{exact:true})).toHaveValue('270');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
});

test('canvas remains usable in a narrow desktop window',async({page})=>{
  await page.setViewportSize({width:1024,height:768});
  await setup(page);
  const side=(await page.getByRole('group',{name:'Холст LCD',exact:true}).boundingBox())!.width;
  await drag(page,'Выбрать слой 3: Часы',10*side/480,20*side/480);
  await expect(layer(page,2).getByLabel('X',{exact:true})).toHaveValue('130');
  await expect(layer(page,2).getByLabel('Y',{exact:true})).toHaveValue('340');
  await page.getByRole('button',{name:'Отменить правку',exact:true}).click();await ready(page);
  await expect(layer(page,2).getByLabel('X',{exact:true})).toHaveValue('120');
});


test('importing an image preserves the selected existing layer',async({page})=>{
  await setup(page);
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="3"]').click();
  await page.getByText('Добавить слой',{exact:true}).click();
  await page.getByLabel('+ Изображение',{exact:true}).setInputFiles({
    name:'new-image.png',mimeType:'image/png',
    buffer:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==','base64')
  });
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(5);
  await ready(page);
  await expect(page.locator('.layer-row[data-layer-index="4"]')).toHaveAttribute('aria-pressed','true');
  await expect(page.getByRole('button',{name:'Выбрать слой 5: Текст',exact:true})).toHaveAttribute('aria-pressed','true');
  const handle=page.getByRole('button',{name:'Изменить размер выбранного слоя',exact:true});
  await handle.focus();await page.keyboard.press('Control+ArrowRight');await ready(page);
  await expect(layer(page,4).getByLabel('X',{exact:true})).toHaveValue('130');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="3"]').click();
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('120');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
});


test('compact stack preserves layer metadata, visibility, lock and explicit display Apply',async({page})=>{
  await setup(page);
  await page.getByText('Добавить слой',{exact:true}).click();
  for(let i=0;i<3;i++)await page.getByRole('button',{name:'+ Текст',exact:true}).click();
  await page.getByText('Добавить слой',{exact:true}).click();
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(7);
  await expect(page.locator('.inspector')).toHaveCount(1);
  await page.locator('.layer-name summary').click();
  await page.getByLabel('Имя слоя',{exact:true}).fill('GPU caption');
  const row=page.locator('.layer-row[data-layer-index="6"]');
  await expect(row).toContainText('GPU caption');
  await page.getByRole('button',{name:'Слои',exact:true}).click();
  await page.getByRole('button',{name:'Блокировка слоя 7',exact:true}).click();
  await expect(page.locator('.inspector').getByLabel('Текст',{exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Блокировка слоя 7',exact:true}).click();
  await page.getByRole('button',{name:'Видимость слоя 7',exact:true}).click();
  await expect(page.getByRole('button',{name:'Выбрать слой 7: Текст',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Дублировать',exact:true}).click();
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(8);
  await page.getByRole('button',{name:'Отменить правку',exact:true}).click();
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(7);
  await page.getByLabel('Ориентация экрана',{exact:true}).selectOption('90');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
  await ready(page);await page.evaluate(()=>{(document.activeElement as HTMLElement)?.blur();window.scrollTo(0,0);});
  await page.screenshot({path:test.info().outputPath('editor-desktop.png'),fullPage:true});
  await page.setViewportSize({width:1024,height:768});
  await page.screenshot({path:test.info().outputPath('editor-narrow.png'),fullPage:true});
  await expect(page.locator('body')).toHaveJSProperty('scrollWidth',1024);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'canvasWrites').length)).toBe(2);
  const writes=await page.evaluate(()=>Reflect.get(window,'canvasWrites'));
  expect(writes[0].theme.layers[6].editor).toMatchObject({name:'GPU caption',hidden:true,locked:false});
  expect(writes[1]).toMatchObject({dev:'test',patch:{rotation_degrees:90}});
});


test('rendered layer follows pointer before committing and Escape restores it',async({page})=>{
  await setup(page);
  const target=page.getByRole('button',{name:'Выбрать слой 4: Текст',exact:true});
  const box=(await target.boundingBox())!;
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+25,box.y+box.height/2+15);
  await expect(page.locator('.layer-pixels')).toHaveCount(4);
  await expect(page.locator('.layer-pixels').last()).toHaveCSS('transform',/matrix\(1, 0, 0, 1, [1-9]/);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('120');
  expect(await page.evaluate(()=>Reflect.get(window,'canvasWrites'))).toEqual([]);
  await page.keyboard.press('Escape');await page.mouse.up();
  await expect(page.locator('.layer-pixels')).toHaveCount(0);
  await expect(layer(page,3).getByLabel('X',{exact:true})).toHaveValue('120');
});

test('loading a smaller document clears stale selection and remains editable',async({page})=>{
  await setup(page);
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="3"]').click();
  await page.evaluate(()=>Reflect.get(window,'canvasReplaceTheme')());
  await page.getByText('Загрузить или остановить',{exact:true}).click();
  await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(1);
  await expect(page.locator('.inspector[data-layer-index="3"]')).toHaveCount(0);
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();
  await expect(layer(page,0).getByLabel('width',{exact:true})).toHaveValue('80');
  await ready(page);
});
