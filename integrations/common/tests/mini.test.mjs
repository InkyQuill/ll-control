import {test} from 'node:test';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
const M=await import(pathToFileURL(process.env.LCONNECT_MINI_MODEL));
function fixture(){
 const sensor=(id,unit,value,label=id)=>({id,unit,value,label,status:'valid',age_ms:0});
 return {version:1,available:true,data:{display:{temperature_unit:'fahrenheit',interface_language:'en-US'},sensors:[sensor('cpu:temperature','celsius',40,'Настройки'),sensor('gpu:A:power','watts',72),sensor('gpu:B:power','watts',99)],defaults:{candidates:[{role:'cpu_temperature',reading:sensor('cpu:temperature','celsius',40)},{role:'gpu_power',reading:sensor('gpu:A:power','watts',72)},{role:'gpu_power',reading:sensor('gpu:B:power','watts',99)}],values:[{role:'cpu_temperature',sensor_id:'cpu:temperature'},{role:'gpu_power',sensor_id:'gpu:A:power'}]}}};
}
function slots(...values){return {version:1,slots:[...values,...Array(6-values.length).fill(null)]};}
test('experimental Mini disclosure is explicit in EN/RU',()=>{
 assert.match(M.text('experimental','en-US'),/^Experimental:.*Use at your own risk\.$/);
 assert.match(M.text('experimental','ru-RU'),/^Экспериментально:.*Используйте на свой риск\.$/);
});
test('same renderer resolves default GPU, explicit GPU and global Fahrenheit while raw labels stay unchanged',()=>{
 const f=fixture(),config=slots({kind:'cpu_temperature',source:null},{kind:'gpu_power',source:'gpu:B:power'},{kind:'gpu_power',source:null});const r=M.render(f,config);
 assert.deepEqual(r.cards.slice(0,3).map(c=>c.values),[['104.0 °F'],['99.0 W'],['72.0 W']]);assert.equal(r.cards[0].sourceLabel,'Настройки');assert.equal(r.cards.length,6);
 f.data.display.interface_language='ru-RU';const ru=M.render(f,config);assert.equal(ru.cards[0].title,'Температура CPU');assert.deepEqual(ru.cards[0].values,['104,0 °F']);
 assert.equal(M.render(null,config,0,'ru-RU').language,'ru-RU');
});
test('missing, duplicate, wrong role, wrong unit, invalid, stale or locally expired source never substitutes GPU B',()=>{
 for(const change of [f=>f.data.sensors.shift(),f=>f.data.sensors.push({...f.data.sensors[0]}),f=>f.data.defaults.candidates[0].role='gpu_temperature',f=>f.data.sensors[0].unit='watts',f=>f.data.sensors[0].status='stale',f=>f.data.sensors[0].value=NaN,f=>f.data.sensors[0].age_ms=null,f=>f.data.sensors[0].age_ms=3001]) {
  const f=fixture();change(f);assert.equal(M.render(f,slots({kind:'cpu_temperature',source:'cpu:temperature'})).cards[0].available,false);
 }
 assert.equal(M.render(fixture(),slots({kind:'gpu_power',source:'missing'})).cards[0].available,false);
 assert.equal(M.render(fixture(),slots({kind:'gpu_power',source:null}),3001).cards[0].available,false);
});
test('RAM and network require complete fresh pairs, FPS stays unavailable and corrupt/future configs are never silently reset',()=>{
 const f=fixture();f.data.sensors.push(...[{id:'proc:memory:used',unit:'bytes',value:1024**3},{id:'proc:memory:utilization',unit:'percent',value:25},{id:'network:card:rx_bytes_per_second',unit:'bytes_per_second',value:1024},{id:'network:card:tx_bytes_per_second',unit:'bytes_per_second',value:2048}].map(s=>({...s,label:s.id,status:'valid',age_ms:0})));
 const cfg=slots({kind:'ram',source:null},{kind:'network',source:'network:card'},{kind:'fps',source:null});let r=M.render(f,cfg);assert.deepEqual(r.cards[0].values,['25.0%','1.0 GiB']);assert.deepEqual(r.cards[1].values,['↓ 1024.0 B/s','↑ 2048.0 B/s']);assert.equal(r.cards[2].available,false);
 f.data.sensors.pop();assert.equal(M.render(f,cfg).cards[1].available,false);
 for(const cfg of ['broken',{version:2,slots:Array(6).fill(null)},{version:1,slots:[]},slots({kind:'fps',source:'fake'}),slots({kind:'cpu_power',source:'bad\nID'})])assert.equal(M.parseConfig(cfg),null);
 assert.equal(M.render(f,'broken').validConfig,false);assert.match(M.render(f,'broken').status,/Reset explicitly/);assert.equal(M.defaultConfig().slots.length,6);
});
test('FPS uses explicit captured-frame source identity, units and freshness, never shared GPU or a replacement log',()=>{
 const f=fixture(),id='mangohud:2f67616d65:1:2:logged_frame_mean',config=slots({kind:'fps',source:id});
 f.data.sensors.push({id,label:'Game CSV',unit:'frames_per_second',value:59.5,status:'valid',age_ms:0});
 assert.deepEqual(M.sources('fps',f),[{id,label:'Game CSV'}]);
 assert.deepEqual(M.render(f,config).cards[0].values,['59.5 FPS']);
 f.data.display.interface_language='ru-RU';assert.deepEqual(M.render(f,config).cards[0].values,['59,5 FPS']);
 f.data.defaults.values.push({role:'fps',sensor_id:id});
 assert.equal(M.render(f,slots({kind:'fps',source:null})).cards[0].available,false);
 for(const change of [s=>s.age_ms=3001,s=>s.status='stale',s=>s.unit='hertz',s=>s.value=NaN,s=>s.value=-1,s=>s.id='mangohud:2f67616d65:1:3:logged_frame_mean']) {
  const changed=structuredClone(f);change(changed.data.sensors.at(-1));assert.equal(M.render(changed,config).cards[0].available,false);
 }
 f.data.sensors.push({...f.data.sensors.at(-1)});assert.equal(M.render(f,config).cards[0].available,false);
});
