import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FlowOverride, needsG36FlowOverrideMigration } from '../src/lib/g36FlowOverrideMigration'
import type { Block, Project } from '../src/lib/types'

const reference:Block=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8'))

test('saved G36 VAVs gain flow override while retaining edited control tuning',()=>{
  const old=structuredClone(reference)
  old.blocks=old.blocks?.filter(block=>!block.name.endsWith('_flow_override_none'))
  old.connections=old.connections?.filter(edge=>!edge.to?.endsWith('.flow_override_code'))
  for(const zone of old.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    zone.inputs=zone.inputs?.filter(port=>port.name!=='flow_override_code')
    zone.blocks=zone.blocks?.filter(block=>!block.name.startsWith('flow_override_'))
    zone.connections=zone.connections?.filter(edge=>!edge.from?.startsWith('flow_override_')
      && !edge.to?.startsWith('flow_override_') && edge.from!=='flow_override_code')
    for(const edge of zone.connections??[]){
      if(edge.to==='airflow_request'||edge.to==='damper_flow_pi.u_s')edge.from='airflow_request_sum.y'
    }
  }
  const pi=old.blocks?.find(block=>block.name==='zone_01')?.blocks?.find(block=>block.name==='damper_flow_pi')
  pi!.parameters!.find(parameter=>parameter.name==='k')!.value=0.73
  const saved={version:1,name:old.name,graph:old,scenario:{} as Project['scenario'],layout:{}} as Project
  assert.equal(needsG36FlowOverrideMigration(saved),true)
  const migrated=migrateG36FlowOverride(saved,reference)
  const zone=migrated.graph.blocks?.find(block=>block.name==='zone_01')
  assert.equal(zone?.blocks?.find(block=>block.name==='damper_flow_pi')?.parameters?.find(parameter=>parameter.name==='k')?.value,0.73)
  assert.equal(zone?.inputs?.find(port=>port.name==='flow_override_code')?.type,'Integer')
  assert.equal(zone?.connections?.some(edge=>edge.from==='flow_override_select.y'&&edge.to==='airflow_request'),true)
  const expectedTargets=reference.blocks?.find(block=>block.name==='zone_01')?.connections
    ?.filter(edge=>edge.from==='flow_override_select.y').map(edge=>edge.to).sort()
  const actualTargets=zone?.connections?.filter(edge=>edge.from==='flow_override_select.y')
    .map(edge=>edge.to).sort()
  assert.deepEqual(actualTargets,expectedTargets)
  assert.equal(migrated.graph.blocks?.filter(block=>block.name.endsWith('_flow_override_none')).length,10)
  assert.equal(needsG36FlowOverrideMigration(migrated),false)
  assert.equal(migrateG36FlowOverride(migrated,reference),migrated)
  const renamed=structuredClone(saved)
  renamed.graph.name='EditedG36Controls'
  assert.equal(needsG36FlowOverrideMigration(renamed),true)
  assert.equal(migrateG36FlowOverride(renamed,reference).graph.name,'EditedG36Controls')
})
