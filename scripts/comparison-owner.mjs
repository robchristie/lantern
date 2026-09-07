#!/usr/bin/env node
// Private disposable browser lifecycle and independent observation, never evaluated input.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {spawn,execFileSync} from 'node:child_process';
import {readFile,writeFile,appendFile,mkdir,rm} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const [mode,runPath]=process.argv.slice(2);
if(!['serve','restart','status','stop'].includes(mode)||!runPath)throw Error('Usage: node scripts/comparison-owner.mjs serve|restart|status|stop RUN_DIR');
const out=resolve(runPath);
if(mode!=='serve'){
 const owner=JSON.parse(await readFile(out+'/owner.json','utf8'));
 const response=await fetch(owner.control+'/'+mode,{method:mode==='status'?'GET':'POST'});
 console.log(await response.text());if(!response.ok)process.exitCode=1;
}else{
 const require=createRequire(root+'/.smoogle/qualification-tools/package.json');
 const {chromium}=require('playwright-core');
 const fixture=root+'/scripts/fixtures/comparison/index.html';
 const executable=process.env.LANTERN_CHROMIUM;
 if(!executable)throw Error('LANTERN_CHROMIUM must name the approved executable');
 await mkdir(out,{recursive:true});
 if(await readFile(out+'/owner.json').then(()=>true,()=>false))throw Error('Use a new run directory');
 const sha=data=>createHash('sha256').update(data).digest('hex');
 const log=async (name,value)=>appendFile(out+'/'+name+'.jsonl',JSON.stringify({utc:new Date().toISOString(),...value})+'\n');
 let browser,proc,generation=0,identity,stopping=false;
 const fixtureServer=createServer(async(req,res)=>{
  if(req.method==='POST'&&req.url==='/truth'){
   let data='';for await(const chunk of req)data+=chunk;
   await log('truth',{generation,event:JSON.parse(data)});res.writeHead(204);res.end();
  }else if(req.url==='/intentional-fast-failure'){
   await log('server',{generation,path:req.url,status:503});res.writeHead(503,{'Content-Type':'text/plain'});res.end('Intentional unavailable service');
  }else if(req.url==='/favicon.ico'){res.writeHead(204);res.end();}
  else{res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store'});res.end(await readFile(fixture));}
 });
 await new Promise(r=>fixtureServer.listen(0,'127.0.0.1',r));
 const fixtureUrl='http://127.0.0.1:'+fixtureServer.address().port;
 async function stopBrowser(){
  if(browser){await browser.close().catch(()=>{});browser=null;}
  if(proc&&proc.exitCode===null){proc.kill('SIGTERM');await Promise.race([new Promise(r=>proc.once('exit',r)),new Promise(r=>setTimeout(r,5000))]);if(proc.exitCode===null){proc.kill('SIGKILL');await new Promise(r=>proc.once('exit',r));}}
 }
 async function launch(){
  generation++;const profile=out+'/profile-'+generation;
  await mkdir(profile,{recursive:true});
  const args=['--headless=new','--remote-debugging-address=127.0.0.1','--remote-debugging-port=0','--user-data-dir='+profile,'--window-size=1280,900','--force-device-scale-factor=1','--hide-scrollbars','--no-first-run','--no-default-browser-check','about:blank'];
  proc=spawn(executable,args,{stdio:['ignore','ignore','pipe'],env:process.env});
  proc.stderr.on('data',data=>appendFile(out+'/chromium-'+generation+'.log',data));
  let port;
  for(let n=0;n<100;n++){try{port=(await readFile(profile+'/DevToolsActivePort','utf8')).split('\n')[0];break;}catch{}if(proc.exitCode!==null)throw Error('Chromium exited '+proc.exitCode);await new Promise(r=>setTimeout(r,100));}
  if(!port)throw Error('Chromium endpoint not ready');
  const endpoint='http://127.0.0.1:'+port;
  browser=await chromium.connectOverCDP(endpoint);
  const context=browser.contexts()[0];
  const watch=async page=>{
   await page.setViewportSize({width:1280,height:900});
   page.on('console',msg=>log('observer',{generation,kind:'console',type:msg.type(),text:msg.text()}));
   page.on('pageerror',err=>log('observer',{generation,kind:'pageerror',message:err.message}));
   page.on('response',response=>{if(response.status()>=400)log('observer',{generation,kind:'http-error',url:response.url(),status:response.status()});});
  };
  for(const page of context.pages())await watch(page);
  context.on('page',watch);
  const page=context.pages()[0];
  const argv=(await readFile('/proc/'+proc.pid+'/cmdline','utf8')).split('\0').filter(Boolean);
  if(argv.join(' ').split(/\s+/).some(a=>a==='--no-sandbox'||a==='--disable-setuid-sandbox'))throw Error('Sandbox disabling flag');
  identity={generation,pid:proc.pid,endpoint,fixtureUrl,version:browser.version(),launchArgs:args,argv,viewport:await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})),initialUrl:page.url(),freshProfile:profile,fixture_sha256:sha(await readFile(fixture)),executable_sha256:sha(await readFile(executable)),lantern:JSON.parse(execFileSync(root+'/target/debug/lantern',['capabilities'],{encoding:'utf8'}))};
  await log('lifecycle',{kind:'started',...identity});
  await writeFile(out+'/browser.json',JSON.stringify(identity,null,2));
 }
 const control=createServer(async(req,res)=>{
  try{
   if(req.url==='/status'){
    const alive=proc&&proc.exitCode===null;
    res.end(JSON.stringify({...identity,alive}));
   }else if(req.method==='POST'&&req.url==='/restart'){
    const previous=identity;await log('lifecycle',{kind:'pre-restart',pid:previous.pid,alive:proc.exitCode===null});await stopBrowser();await launch();await log('lifecycle',{kind:'restarted',oldPid:previous.pid,newPid:identity.pid});res.end(JSON.stringify(identity));
   }else if(req.method==='POST'&&req.url==='/stop'){
    await log('lifecycle',{kind:'pre-stop',pid:identity.pid,alive:proc.exitCode===null});await stopBrowser();res.end(JSON.stringify({stopped:true,pid:identity.pid}));stopping=true;control.close();fixtureServer.close();for(let n=1;n<=generation;n++)await rm(out+'/profile-'+n,{recursive:true,force:true,maxRetries:5,retryDelay:100});
   }else{res.writeHead(404);res.end();}
  }catch(error){res.writeHead(500);res.end(JSON.stringify({error:String(error)}));}
 });
 await launch();
 await new Promise(r=>control.listen(0,'127.0.0.1',r));
 await writeFile(out+'/owner.json',JSON.stringify({pid:process.pid,control:'http://127.0.0.1:'+control.address().port,fixtureUrl},null,2));
 console.log(JSON.stringify(identity));
 const shutdown=async()=>{if(stopping)return;stopping=true;await stopBrowser();control.close();fixtureServer.close();};
 process.on('SIGTERM',shutdown);process.on('SIGINT',shutdown);
}
