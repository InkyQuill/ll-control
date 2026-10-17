#!/usr/bin/env node
// Observe user-supplied Windows assets locally; never copy them into the project.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join,extname,sep} from 'node:path';
import {createHash} from 'node:crypto';
const require=createRequire(new URL('../desktop/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const [reference,output]=process.argv.slice(2);
if(!reference||!output)throw new Error('Usage: inspect-windows-grid.mjs REFERENCE_GA2V_DIRECTORY OUTPUT_DIRECTORY');
const root=resolve(reference),dest=resolve(output);await mkdir(dest,{recursive:true});
const index=await readFile(join(root,'index.html'),'utf8');
const main=index.match(/src="(js\/app\.[a-f0-9]+\.js)"/)?.[1];
if(!main)throw new Error('Missing shipped main bundle');
const original=await readFile(join(root,main),'utf8');
const activeCss=index.match(/href="(css\/app\.[a-f0-9]+\.css)"/)?.[1];
if(!activeCss)throw new Error('Missing shipped stylesheet');
const sha=value=>createHash('sha256').update(value).digest('hex');
if(sha(await readFile(join(root,activeCss)))!=='c0f1d6d5bb1febfe0503b78a8ee23c3416cee0c044a3d4513e4f9342fa4357ec')throw new Error('Expected frozen2.1.28 stylesheet');
if(sha(original)!=='f3691c15dbe62d1248abee444dc94c82a34c46e45697bda4ccad77825c6e878a')throw new Error('Expected frozen2.1.28 GA2V bundle');
const bootstrap='const cT=(0,n.ri)(u);cT.use(uT.Z),cT.use(sT),cT.use(oT),cT.mount("#app")';
if(original.split(bootstrap).length!==2)throw new Error('Unexpected reference bootstrap');
const patched=original.replace(bootstrap,`window.mountOwnedGrid=(count,font,mode)=>{
 window.ownedGrid?.unmount();
 const values={CPUTemperature:{Type:'CPU',Value:'42.0',Unit:'°C'},GPUTemperature:{Type:'GPU',Value:'61.0',Unit:'°C'},PumpRPM:{Type:'PUMP',Value:'2200',Unit:'RPM'},CoolantTemp:{Type:'COOL',Value:'35.0',Unit:'°C'}};
 const props={effectMode:mode,textData:{Type:count===3?'ThreeSensor':'FourSensor',Sensors:Object.keys(values).slice(0,count),Layout:[{Shadow:20,FontFamily:font,FontColors:[[235,245,255],[71,201,137]]}]},sensorValue:values};
 window.ownedGrid=(0,n.ri)(count===3?Cn:Bn,props);window.ownedGrid.mount('#app');
}`);
const served=new Map();
const server=createServer(async(req,res)=>{
 try{
  const path=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));
  if(!path.startsWith(root+sep)&&path!==root)throw new Error('Outside reference root');
  let bytes=path===join(root,main)?Buffer.from(patched):await readFile(path===root?join(root,'index.html'):path);
  served.set(path===root?'index.html':path.slice(root.length+1),sha(path===join(root,main)?original:bytes));
  if(path===root||path===join(root,'index.html'))bytes=Buffer.from(bytes.toString().replace('</head>','<style>html,body,#app{margin:0;width:480px;height:480px;overflow:hidden}</style></head>'));
  const mime={'.js':'text/javascript','.css':'text/css','.html':'text/html','.ttf':'font/ttf','.otf':'font/otf','.woff':'font/woff','.woff2':'font/woff2','.png':'image/png'};
  res.writeHead(200,{'Content-Type':mime[extname(path)]??(path===root?'text/html':'application/octet-stream'),'Cache-Control':'no-store'});res.end(bytes);
 }catch{res.writeHead(404);res.end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin=`http://127.0.0.1:${server.address().port}`;
let browser;
try{
 browser=await chromium.launch({headless:true});
 const page=await browser.newPage({viewport:{width:480,height:480},deviceScaleFactor:1});
 const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.route('**/*',route=>route.request().url().startsWith(origin+'/')?route.continue():route.abort());
 await page.goto(origin+'/');await page.waitForFunction(()=>typeof window.mountOwnedGrid==='function');
 const results=[];
 for(const mode of ['Image','Color'])for(const count of [3,4])for(const font of ['HyperspaceRace_BoldItalic','NotoSansTC_Regular','Caprasimo_Regular']){
  await page.evaluate(({count,font,mode})=>window.mountOwnedGrid(count,font,mode),{count,font,mode});
  await page.evaluate(()=>document.fonts.ready);await page.locator(count===3?'.text_three_sensor_wrap':'.text_four_sensor_wrap').waitFor();
  const result=await page.evaluate(()=>{
   const root=document.getElementById('app').getBoundingClientRect();
   const dividers=[...document.querySelectorAll('.border_line_h,.border_line_v')].map(node=>{
    const r=node.getBoundingClientRect(),s=getComputedStyle(node);
    return {kind:node.className,x:r.x-root.x,y:r.y-root.y,width:r.width,height:r.height,color:s.backgroundColor,shadow:s.boxShadow};
   });
   return {dividers,loadedFontFaces:[...document.fonts].filter(face=>face.status==='loaded').map(face=>face.family),valueFonts:[...document.querySelectorAll('.main_font_size')].map(node=>{const s=getComputedStyle(node);return {family:s.fontFamily,size:s.fontSize};})};
  });
  results.push({mode,count,font,...result});await page.screenshot({path:join(dest,`grid-${mode}-${count}-${font}.png`)});
 }
 const css=index.match(/href="(css\/app\.[a-f0-9]+\.css)"/)?.[1];
 const report={scope:'Original frozen Vue components and shipped CSS, owned sensor values in Linux Chromium; no Windows/WPF/hardware claim',reference:root,main_sha256:sha(original),css_sha256:sha(await readFile(join(root,css))),bootstrap_override:'Only replace root application mount with original Cn/Bn component mounts; no component or CSS changes',served_reference_files_sha256:Object.fromEntries(served),results,errors};
 await writeFile(join(dest,'geometry.json'),JSON.stringify(report,null,2)+'\n');
 if(errors.length)throw new Error(JSON.stringify(errors));
 console.log(JSON.stringify(results));
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
