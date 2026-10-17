import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,symlinkSync,realpathSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {translate} from '../src/lib/localization.ts';

test('unknown keys that name inherited object properties remain literal UI fallback text',()=>{
 for(const language of ['en-US','ru-RU'])for(const key of ['constructor','toString','__proto__','hasOwnProperty','valueOf']){
  assert.equal(translate(key,language),key);
  assert.equal(translate(` ${key} `,language),` ${key} `);
 }
});
test('catalog resources interpolate filenames as literal values',()=>{
 const name='constructor {0} <clip>';
 assert.equal(translate('Первый кадр: {0}','en-US',name),'First frame: '+name);
 assert.equal(translate('Первый кадр: {0}','ru-RU',name),'Первый кадр: '+name);
});
test('strict catalog audit requires own dictionary entries and reports original TypeScript lines',()=>{
 const root=mkdtempSync(join(tmpdir(),'lconnect-locale-contract-'));
 try{
  const scripts=join(root,'desktop/scripts'),lib=join(root,'desktop/src/lib'),routes=join(root,'desktop/src/routes');
  mkdirSync(scripts,{recursive:true});mkdirSync(join(lib,'locales'),{recursive:true});mkdirSync(routes,{recursive:true});
  symlinkSync(realpathSync(new URL('../node_modules',import.meta.url)),join(root,'desktop/node_modules'),'dir');
  writeFileSync(join(scripts,'check-locales.mjs'),readFileSync(new URL('./check-locales.mjs',import.meta.url)));
  writeFileSync(join(routes,'+page.svelte'),'<p>Fixture</p>\n');
  const keys=['constructor','toString','__proto__'];
  writeFileSync(join(lib,'owned.ts'),'// original module line mapping\n'+keys.map(k=>`throw new ValidationError(${JSON.stringify(k)});`).join('\n'));
  const dictionary=join(lib,'locales/en-US.json'),report=join(root,'report.json');writeFileSync(dictionary,'{}');
  const run=()=>spawnSync(process.execPath,[join(scripts,'check-locales.mjs'),'--strict','--report',report],{encoding:'utf8'});
  const missing=run();assert.equal(missing.status,1,missing.stderr);const result=JSON.parse(readFileSync(report,'utf8'));assert.equal(result.missingStaticKeys,3);
  for(const [index,key]of keys.entries())assert.deepEqual(result.missing.find(r=>r.key===key).locations,[{file:'desktop/src/lib/owned.ts',line:index+2}]);
  writeFileSync(dictionary,JSON.stringify(Object.fromEntries(keys.map(k=>[k,`Own ${k} entry`]))));
  const translated=run();assert.equal(translated.status,0,translated.stderr);assert.equal(JSON.parse(readFileSync(report,'utf8')).missingStaticKeys,0);
 }finally{rmSync(root,{recursive:true,force:true});}
});
