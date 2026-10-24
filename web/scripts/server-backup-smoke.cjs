/* Production frontend + intercepted APIs only. No backup/agent/provider is run. */
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const base=process.env.QONPANEL_UI_URL||'http://127.0.0.1:5196';
const output=path.join(__dirname,'../artifacts/server-backup');
const password='fixture backup passphrase only';
let checks=0;
const check=(condition,message)=>{assert.ok(condition,message);console.log(`ok ${++checks}: ${message}`);};
(async()=>{
 await fs.mkdir(output,{recursive:true});
 const browser=await chromium.launch({headless:true,channel:'chrome'});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:1100},reducedMotion:'reduce'});
  page.setDefaultTimeout(15000);
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  let state={supported:true,job:null,backups:[],inspection:null};
  const jobs=[],chunks=[];let loseRestoreAck=false,offset=0,deleted=0;
  await page.addInitScript(()=>{Object.defineProperty(globalThis.crypto,'randomUUID',{value:undefined});if(!localStorage.getItem('nusa_theme'))localStorage.setItem('nusa_theme','light');});
  await page.route('**/api/v1/**',async route=>{
   const request=route.request(),url=new URL(request.url());let status=200,body={};
   if(url.pathname==='/api/v1/auth/me')body={user:{id:1,username:'admin',role:'admin'},csrf_token:'fixture-csrf'};
   else if(url.pathname==='/api/v1/auth/csrf')body={csrf_token:'fixture-csrf'};
   else if(url.pathname==='/api/v1/system/server-backup')body=state;
   else if(url.pathname==='/api/v1/system/server-backup/jobs'){
    assert.equal(request.headers()['x-csrf-token'],'fixture-csrf');
    const payload=request.postDataJSON();jobs.push(payload);
    assert.equal(payload.passphrase,password);
    state={...state,job:{id:payload.id,operation:payload.operation,phase:'queued',message:'Pekerjaan masuk antrean.'}};
    if(payload.operation==='restore'&&loseRestoreAck){await route.abort('failed');return;}
    status=202;body=state;
   }
   else if(url.pathname.includes('/server-backup/uploads/')){
    if(request.method()==='DELETE'){body={};status=204;}
    else {const start=Number(url.searchParams.get('offset')),total=Number(url.searchParams.get('total'));assert.equal(start,offset);const count=request.postDataBuffer().length;assert.ok(count<=8*1024*1024);chunks.push(count);offset+=count;body={offset,total,complete:offset===total};}
   }
   else if(url.pathname.includes('/server-backup/archives/')){
    if(request.method()==='DELETE'){deleted++;state={...state,backups:[]};body=state;}
    else {await route.fulfill({status:200,contentType:'application/octet-stream',headers:{'Content-Disposition':'attachment; filename="QonPanel-backup-fixture.qpb"'},body:'encrypted transfer fixture only'});return;}
   }
   else if(url.pathname.includes('notifications'))body={notifications:[],count:0,unread_count:0,total:0};
   else if(url.pathname.includes('jobs'))body={items:[],total:0};
   else {status=404;body={detail:'No fixture'};}
   await route.fulfill({status,contentType:'application/json',body:status===204?undefined:JSON.stringify(body)});
  });
  await page.goto(`${base}/settings?section=server_backup`);
  await page.getByRole('heading',{name:'Backup dan migrasi server',exact:true}).waitFor();
  check(await page.getByRole('button',{name:'Buat backup penuh',exact:true}).isDisabled(),'empty/short passphrases cannot start a full backup');
  const secret=()=>page.getByLabel('Passphrase backup',{exact:true});
  await secret().fill(password);await page.getByLabel('Ulangi passphrase untuk backup baru',{exact:true}).fill(password);
  await page.getByRole('button',{name:'Buat backup penuh',exact:true}).click();
  await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'hidden'});
  check(jobs.length===0&&!await page.getByRole('dialog').count(),'Escape cancels snapshot confirmation without submitting');
  await page.getByRole('button',{name:'Buat backup penuh',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Lanjutkan',exact:true}).click();
  await page.getByText('Pekerjaan dijadwalkan',{exact:true}).waitFor();
  check(jobs.length===1&&jobs[0].operation==='create','backup is submitted once as a typed create job');
  check(await secret().inputValue()==='','accepted jobs clear the passphrase from the form');
  check(!await page.evaluate(()=>JSON.stringify({...sessionStorage,...localStorage}).includes('fixture backup passphrase')),'browser storage contains no passphrase');
  state={...state,job:{...state.job,phase:'running',message:'Membuat snapshot layanan.'}};
  await page.reload();await page.getByText('Pekerjaan berjalan',{exact:true}).waitFor();
  check(await page.getByRole('button',{name:'Buat backup penuh',exact:true}).isDisabled(),'server progress survives reload and prevents duplicate work');
  state={...state,job:{...state.job,phase:'succeeded',message:'Backup siap diunduh.'},backups:[{id:jobs[0].id,size:4096,created_at:Math.floor(Date.now()/1000),digest:'a'.repeat(64),hostname:'old-server',panel_version:'0.1.0'}]};
  await page.getByRole('link',{name:'Unduh',exact:true}).waitFor();
  const [download]=await Promise.all([page.waitForEvent('download'),page.getByRole('link',{name:'Unduh',exact:true}).click()]);
  check(download.suggestedFilename().startsWith('QonPanel-backup'),'download offers the QonPanel encrypted archive filename');
  await page.getByRole('button',{name:'Hapus',exact:true}).click();await page.keyboard.press('Escape');
  check(deleted===0,'archive deletion requires a separate confirmation');
  await page.locator('input[type=file]').setInputFiles({name:'migration.qpb',mimeType:'application/octet-stream',buffer:Buffer.alloc(9*1024*1024,0x42)});
  await page.getByRole('button',{name:'Unggah arsip',exact:true}).click();
  await page.getByText(/100% terunggah/).waitFor();
  check(chunks.length===2&&chunks[0]===8*1024*1024&&chunks[1]===1024*1024,'large encrypted files are transferred in bounded eight-MiB chunks');
  await secret().fill(password);await page.getByRole('button',{name:'Periksa arsip',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Lanjutkan',exact:true}).click();
  await page.getByText('Pekerjaan dijadwalkan',{exact:true}).waitFor();
  const inspectionJob=jobs.at(-1);assert.equal(inspectionJob.operation,'inspect');
  state={...state,job:{...state.job,phase:'succeeded',message:'Pemeriksaan selesai.'},inspection:{archive_id:inspectionJob.archive_id,digest:'b'.repeat(64),hostname:'old-server',panel_version:'0.1.0',tenant_count:2,uncompressed_bytes:99999,packages:['php8.4-fpm','mariadb-server'],checked_at:Math.floor(Date.now()/1000),warnings:['DNS harus menunjuk server baru.']}};
  await page.getByText('Backup dari old-server',{exact:true}).waitFor();
  check(await page.getByRole('button',{name:'Restore ke server ini',exact:true}).isDisabled(),'restore remains disabled until the passphrase is entered again');
  await page.reload();await page.getByText('Backup dari old-server',{exact:true}).waitFor();
  check(await secret().inputValue()==='','inspection and upload identity survive reload without saving the passphrase');
  await secret().fill(password);
  check(await page.getByRole('button',{name:'Restore ke server ini',exact:true}).isDisabled(),'privileged configuration cannot be restored without out-of-band root hash approval');
  await page.getByText('Pengesahan pemilik server diperlukan',{exact:true}).waitFor();
  state={...state,approval:{digest:state.inspection.digest,expires_at:Math.floor(Date.now()/1000)+86400}};
  await page.getByRole('button',{name:'Restore ke server ini',exact:true}).click();
  check(await page.getByRole('dialog').getByRole('button',{name:'Restore sekarang',exact:true}).isDisabled(),'restore requires destination admin password and explicit confirmation');
  await page.keyboard.press('Escape');
  check(jobs.filter(job=>job.operation==='restore').length===0,'closing the restore dialog does not replace any data');
  await page.getByRole('button',{name:'Restore ke server ini',exact:true}).click();
  await page.getByLabel('Password administrator server ini',{exact:true}).fill('destination-admin-fixture');
  await page.getByRole('checkbox').check();loseRestoreAck=true;
  await page.getByRole('dialog').getByRole('button',{name:'Restore sekarang',exact:true}).click();
  await page.getByText('Pekerjaan dijadwalkan',{exact:true}).waitFor();
  await page.reload();await page.getByText('Pekerjaan dijadwalkan',{exact:true}).waitFor();
  check(jobs.filter(job=>job.operation==='restore').length===1,'lost restore acknowledgement and reload do not resubmit the restore');
  check(jobs.at(-1).digest===state.inspection.digest&&jobs.at(-1).confirm_restore===true,'restore pins the inspected archive digest and confirmation');
  check(!await page.evaluate(()=>JSON.stringify({...sessionStorage,...localStorage}).includes('destination-admin-fixture')),'destination administrator password is never persisted');
  state={...state,job:{...state.job,phase:'failed',message:'Pemulihan snapshot belum selesai; gunakan SSH untuk pemulihan keselamatan.'}};
  await page.getByText('Pekerjaan belum berhasil',{exact:true}).waitFor();
  check(await page.getByText(/Pemulihan snapshot belum selesai/).isVisible(),'recovery failure is shown without claiming a successful rollback');
  await page.screenshot({path:path.join(output,'desktop.png')});
  await page.setViewportSize({width:320,height:740});
  check(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'backup and restore fit a 320px viewport');
  await page.screenshot({path:path.join(output,'mobile.png')});
  await page.evaluate(()=>{localStorage.setItem('nusa_theme','dark');location.reload();});await page.getByRole('heading',{name:'Backup dan migrasi server',exact:true}).waitFor();
  check(await page.locator('html').evaluate(el=>el.classList.contains('dark')),'backup page follows the existing dark theme preference');
  await page.screenshot({path:path.join(output,'dark-mobile.png')});
  check(errors.length===0,'no browser runtime exceptions');
  await fs.writeFile(path.join(output,'results.json'),JSON.stringify({checks,chunks,operations:jobs.map(job=>({id:job.id,operation:job.operation})),errors},null,2));
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
