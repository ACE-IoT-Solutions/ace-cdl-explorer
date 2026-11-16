import { expect, test, type Page } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { gunzipSync } from 'node:zlib'

async function observeWorker(page: Page) {
  await page.addInitScript(() => {
    const OriginalWorker = window.Worker
    window.Worker = class extends OriginalWorker {
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url,options)
        this.addEventListener('message',({data}) => {
          if(data.type==='batch') (window as any).__simulationBatch={progress:data.progress,total:data.total,done:data.done,summary:data.summary}
          if(data.type==='error') (window as any).__simulationError=data.message
        })
      }
    }
  })
}
async function importProject(page:Page,program:'building'|'g36') {
  await page.goto('/#editor')
  await page.getByRole('button',{name:'Visual editor',exact:true}).click()
  const file = new URL(`../fixtures/pre-workspace-four_pipe_${program}.ace-cdl.json${program==='g36'?'.gz':''}`,import.meta.url)
  const contents=program==='g36'?gunzipSync(readFileSync(file)):readFileSync(file)
  await page.locator('input[type=file]').setInputFiles({name:`saved-${program}.ace-cdl.json`,mimeType:'application/json',buffer:contents})
  await expect(page.getByRole('heading',{name:program==='g36'?'FourPipeG36Controls':'FourPipeBuildingControls',exact:true})).toBeVisible({timeout:30_000})
  await page.getByRole('button',{name:'Simulation',exact:true}).click()
  await expect(page.getByRole('button',{name:'Run 2025 simulation',exact:true})).toBeEnabled()
}
for(const program of ['building','g36'] as const) {
  test(`pre-workspace ${program} export loads and runs through the browser worker`,async({page})=>{
    const errors:string[]=[]
    page.on('pageerror',error=>errors.push(error.message))
    await observeWorker(page)
    await importProject(page,program)
    await page.getByRole('button',{name:'Run 2025 simulation',exact:true}).click()
    await page.waitForFunction(()=>Boolean((window as any).__simulationBatch||(window as any).__simulationError))
    const batch=await page.evaluate(()=>(window as any).__simulationBatch)
    expect(await page.evaluate(()=>(window as any).__simulationError)).toBeUndefined()
    expect(batch.progress).toBeGreaterThan(0)
    expect(batch.total).toBe(program==='g36'?2_102_400:35_040)
    const cancel=page.getByRole('button',{name:'Cancel',exact:true})
    if(await cancel.isVisible())await cancel.click()
    expect(errors).toEqual([])
  })
}
test('G36 full-year browser summary matches the pre-workspace browser',async({page})=>{
  test.skip(process.env.CDL_BROWSER_ANNUAL!=='1','Explicit full-year browser gate')
  test.setTimeout(600_000)
  const expected=JSON.parse(readFileSync(new URL('../fixtures/pre-workspace-browser-g36-summary.json',import.meta.url),'utf8'))
  const errors:string[]=[]
  page.on('pageerror',error=>errors.push(error.message))
  await observeWorker(page)
  await importProject(page,'g36')
  await page.getByRole('button',{name:'Run 2025 simulation',exact:true}).click()
  await page.waitForFunction(()=>JSON.parse(localStorage.getItem('ace-cdl-run-history-v1')??'[]').some((r:any)=>r.program==='g36')||(window as any).__simulationError,null,{timeout:540_000})
  expect(await page.evaluate(()=>(window as any).__simulationError)).toBeUndefined()
  const run=await page.evaluate(()=>JSON.parse(localStorage.getItem('ace-cdl-run-history-v1')??'[]').find((r:any)=>r.program==='g36'))
  expect(run.summary).toEqual(expected.summary)
  expect(run.effectiveStepSeconds).toBe(expected.effective_step_seconds)
  expect((await page.evaluate(()=>(window as any).__simulationBatch)).done).toBe(true)
  expect(errors).toEqual([])
})

test('generated glossary and palette expose availability and retain weighted Add',async({page})=>{
  const errors:string[]=[]
  page.on('pageerror',error=>errors.push(error.message))
  await page.goto('/#glossary')
  await page.getByRole('button',{name:'Glossary',exact:true}).click()
  await page.getByLabel('Find a block').fill('MatrixMax')
  await expect(page.locator('.block-glossary-card')).toHaveCount(1)
  await expect(page.getByText('Import only',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Visual editor',exact:true}).click()
  await page.locator('.palette-item').filter({hasText:'Weighted add'}).click()
  await expect(page.getByLabel('k1',{exact:true})).toHaveValue('1')
  await expect(page.getByLabel('k2',{exact:true})).toHaveValue('1')
  expect(errors).toEqual([])
})
