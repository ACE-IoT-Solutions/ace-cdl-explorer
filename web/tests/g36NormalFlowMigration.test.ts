import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36NormalFlow, needsG36NormalFlowMigration } from '../src/lib/g36NormalFlowMigration'
import type { Block, Project } from '../src/lib/types'

const reference:Block=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8'))
const added=new Set(['normal_cooling_signal','normal_cooling_limit','normal_heating_signal','ventilation_override_or_disabled','normal_airflow_sequence','normal_warmup_select'])

test('saved G36 zones gain the normal OBC airflow path while retaining tuning and layout',()=>{
  const old=structuredClone(reference)
  for(const zone of old.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    zone.blocks=zone.blocks?.filter(block=>!added.has(block.name))
    zone.connections=zone.connections?.filter(edge=>!added.has(edge.to.split('.')[0])&&!added.has(edge.from.split('.')[0]))
    zone.connections?.push(
      {from:'airflow_request_sum.y',to:'flow_override_select.u2'},
      {from:'ventilation_standby_disabled.y',to:'ventilation_area_presence_or_required.u2'},
    )
  }
  const first=old.blocks?.find(block=>block.name==='zone_01')
  first!.blocks!.find(block=>block.name==='cool_pi')!.parameters!.find(parameter=>parameter.name==='k')!.value=0.73
  first!.blocks!.find(block=>block.name==='flow_span')!.parameters!.find(parameter=>parameter.name==='k')!.value=0.55
  const second=old.blocks?.find(block=>block.name==='zone_02')
  second!.blocks!.find(block=>block.name==='flow_span')!.parameters!.find(parameter=>parameter.name==='k')!.value=0.55
  second!.blocks!.find(block=>block.name==='flow_override_cooling_max')!.parameters!.find(parameter=>parameter.name==='k')!.value=0.85
  const saved={version:1,name:old.name,graph:old,scenario:{} as Project['scenario'],layout:{zone_01:{x:100,y:200}}} as Project
  assert.equal(needsG36NormalFlowMigration(saved),true)
  const migrated=migrateG36NormalFlow(saved,reference)
  const zone=migrated.graph.blocks?.find(block=>block.name==='zone_01')
  const parameter=(name:string,parameter:string)=>zone?.blocks?.find(block=>block.name===name)?.parameters?.find(item=>item.name===parameter)?.value
  assert.equal(parameter('cool_pi','k'),0.73)
  assert.equal(parameter('flow_override_cooling_max','k'),0.75)
  assert.equal(zone?.connections?.some(edge=>edge.from==='normal_warmup_select.y'&&edge.to==='flow_override_select.u2'),true)
  assert.equal(zone?.connections?.some(edge=>edge.from==='heat_pi.y'&&edge.to==='heating_damper_gain.u'),true)
  assert.equal(zone?.connections?.some(edge=>edge.from==='ventilation_override_or_disabled.y'&&edge.to==='ventilation_area_presence_or_required.u2'),true)
  assert.equal(zone?.connections?.some(edge=>edge.from==='discharge_temp'&&edge.to==='normal_airflow_sequence.discharge_temp'),true)
  assert.equal(migrated.graph.blocks?.find(block=>block.name==='zone_02')?.blocks?.find(block=>block.name==='flow_override_cooling_max')?.parameters?.find(parameter=>parameter.name==='k')?.value,0.85)
  assert.deepEqual(migrated.layout.zone_01,{x:100,y:200})
  assert.equal(needsG36NormalFlowMigration(migrated),false)
  assert.equal(migrateG36NormalFlow(migrated,reference),migrated)
})
