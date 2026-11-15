// One-time baseline capture against an isolated, pre-migration Explorer checkout.
import { createHash } from 'node:crypto'
import { chromium } from '@playwright/test'
import { writeFileSync, readFileSync, unlinkSync } from 'node:fs'
import { gzipSync, gunzipSync } from 'node:zlib'
const browser = await chromium.launch({ args: ['--disable-background-timer-throttling', '--disable-renderer-backgrounding'] })
const page = await browser.newPage()
page.on('pageerror', error => console.error('PAGE ERROR', error.message))
try {
  const baseURL=process.env.BASELINE_URL ?? 'http://127.0.0.1:4182/#editor'
  const wasmBytes=Buffer.from(await(await fetch(new URL('/src/wasm/ace_cdl_runtime_bg.wasm',baseURL))).arrayBuffer())
  await page.goto(baseURL)
  await page.getByRole('button', {name:'Visual editor',exact:true}).click()
  for (const [file, name] of [['four_pipe_building.json','FourPipeBuildingControls'],['four_pipe_g36.json','FourPipeG36Controls']]) {
    await page.getByLabel('Load example').selectOption(file)
    await page.getByRole('button',{name:'Load',exact:true}).click()
    await page.getByRole('heading',{name,exact:true}).waitFor()
    const download = page.waitForEvent('download')
    await page.getByRole('button',{name:'Export project',exact:true}).click()
    const destination=new URL(`../fixtures/pre-workspace-${file.replace('.json','')}.ace-cdl.json`,import.meta.url)
    await (await download).saveAs(destination.pathname)
    if(file==='four_pipe_g36.json') { writeFileSync(new URL(destination.href+'.gz'),gzipSync(readFileSync(destination),{level:9,mtime:0})); unlinkSync(destination) }
    console.log(`Captured original UI export: ${file}`)
  }
  await page.getByRole('button',{name:'Simulation',exact:true}).click()
  await page.getByRole('button',{name:'Run 2025 simulation',exact:true}).click()
  console.log('Original browser G36 annual run started')
  await page.waitForFunction(() => JSON.parse(localStorage.getItem('ace-cdl-run-history-v1') ?? '[]').some(r=>r.program==='g36'),null,{timeout:600000})
  const run = await page.evaluate(()=>JSON.parse(localStorage.getItem('ace-cdl-run-history-v1')).find(r=>r.program==='g36'))
  writeFileSync(new URL('../fixtures/pre-workspace-browser-g36-summary.json',import.meta.url),JSON.stringify({source_commit:'73f562fc99528181c24cea8b24d81c8a18cdb41f',browser:browser.version(),source_wasm_sha256:createHash('sha256').update(wasmBytes).digest('hex'),source_project_sha256:createHash('sha256').update(gunzipSync(readFileSync(new URL('../fixtures/pre-workspace-four_pipe_g36.ace-cdl.json.gz',import.meta.url)))).digest('hex'),requested_step_seconds:run.requestedStepSeconds,effective_step_seconds:run.effectiveStepSeconds,summary:run.summary},null,2)+'\n')
  console.log(JSON.stringify(run))
} finally { await browser.close() }
