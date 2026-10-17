/** Report explicit resource coverage; dynamic UI messages require manual qualification. */
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parse} from 'svelte/compiler';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const dictionary=JSON.parse(fs.readFileSync(path.join(root,'desktop/src/lib/locales/en-US.json'),'utf8'));
const files=fs.readdirSync(path.join(root,'desktop/src/lib')).filter(f=>f.endsWith('.svelte')||f.endsWith('.ts')).map(f=>'desktop/src/lib/'+f).concat(['desktop/src/routes/+page.svelte']);
const keys=new Map(),dynamic=[];
for(const file of files){const module=file.endsWith('.ts'),original=fs.readFileSync(path.join(root,file),'utf8');const source=module?`<script lang="ts">\n${original}\n</script>`:original;const ast=parse(source,{modern:true});
 function walk(node){if(!node||typeof node!=='object')return;
  if((node.type==='CallExpression'&&node.callee?.type==='Identifier'&&node.callee.name==='t')||
     (node.type==='NewExpression'&&node.callee?.type==='Identifier'&&node.callee.name==='ValidationError')){
   const first=node.arguments[0],location={file,line:source.slice(0,node.start).split('\n').length-(module?1:0)};
   if(first?.type==='Literal'&&typeof first.value==='string'){const key=first.value.trim().replace(/\s+/g,' ');keys.set(key,[...(keys.get(key)??[]),location]);}
   else dynamic.push({...location,expression:source.slice(first?.start??node.start,first?.end??node.end)});
  }
  for(const [key,value]of Object.entries(node))if(!['metadata','loc'].includes(key)&&value&&typeof value==='object'){if(Array.isArray(value))value.forEach(walk);else walk(value);}
 }
 walk(ast.fragment);walk(ast.instance);
}
const holes=text=>[...text.matchAll(/\{(\d+)\}/g)].map(m=>m[1]).sort();
for(const [key,value]of Object.entries(dictionary)){
 if(typeof value!=='string'||!value.trim())throw new Error('Empty resource: '+key);
 if(JSON.stringify(holes(key))!==JSON.stringify(holes(value)))throw new Error('Parameter mismatch: '+key);
}
const missing=[...keys].filter(([key])=>!Object.hasOwn(dictionary,key)).map(([key,locations])=>({key,locations}));
const result={supported:['ru-RU','en-US'],englishStatus:missing.length?'partial':'static-catalog-complete',staticKeys:keys.size,translatedStaticKeys:keys.size-missing.length,missingStaticKeys:missing.length,englishEntries:Object.keys(dictionary).length,missing,dynamicCalls:dynamic,remainingWindowsLocales:['de-DE','es-ES','fr-FR','ko-KR','zh-CN','zh-TW'],notes:['User names, device labels and LCD text bypass translation.','Dynamic messages/labels and native tray/menu require separate qualification.','Static resource completeness alone does not prove runtime or Windows parity.']};
const index=process.argv.indexOf('--report');if(index>=0){const output=path.resolve(process.argv[index+1]);fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify(result,null,2)+'\n');}
console.log(`English static resources: ${result.translatedStaticKeys}/${result.staticKeys}; ${result.missingStaticKeys} missing; ${dynamic.length} dynamic outputs to qualify`);
if(process.argv.includes('--strict')&&missing.length)process.exitCode=1;
