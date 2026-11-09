import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DischargeMinimum, needsG36DischargeMinimumMigration } from '../src/lib/g36DischargeMinimumMigration'
import type { Block, Project } from '../src/lib/types'

const reference:Block=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8'))

test('saved G36 zones gate minimum-discharge reheat by occupied mode without replacing tuning',()=>{
  const old=structuredClone(reference)
  for(const zone of old.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    zone.blocks=zone.blocks?.filter(block=>block.name!=='discharge_minimum_occupied')
    zone.connections=zone.connections?.filter(edge=>edge.to!=='discharge_minimum_occupied.u1'
      &&edge.to!=='discharge_minimum_occupied.u2'&&edge.from!=='discharge_minimum_occupied.y')
    zone.connections?.push({from:'discharge_below_minimum.y',to:'discharge_valve_enable.u2'})
  }
  const first=old.blocks?.find(block=>block.name==='zone_01')
  first!.blocks!.find(block=>block.name==='discharge_valve_pi')!.parameters!.find(parameter=>parameter.name==='k')!.value=0.11
  const saved={version:1,name:old.name,graph:old,scenario:{} as Project['scenario'],layout:{zone_01:{x:123,y:456}}} as Project
  assert.equal(needsG36DischargeMinimumMigration(saved),true)
  const migrated=migrateG36DischargeMinimum(saved,reference)
  const zone=migrated.graph.blocks?.find(block=>block.name==='zone_01')
  assert.equal(zone?.blocks?.find(block=>block.name==='discharge_valve_pi')?.parameters?.find(parameter=>parameter.name==='k')?.value,0.11)
  assert.equal(zone?.connections?.some(edge=>edge.from==='local_occupied.y'&&edge.to==='discharge_minimum_occupied.u2'),true)
  assert.equal(zone?.connections?.some(edge=>edge.from==='discharge_minimum_occupied.y'&&edge.to==='discharge_valve_enable.u2'),true)
  assert.deepEqual(migrated.layout.zone_01,{x:123,y:456})
  assert.equal(needsG36DischargeMinimumMigration(migrated),false)
  assert.equal(migrateG36DischargeMinimum(migrated,reference),migrated)
})
