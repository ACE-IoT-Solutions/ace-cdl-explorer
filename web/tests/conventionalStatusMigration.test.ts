import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateConventionalStatus, needsConventionalStatusMigration } from '../src/lib/conventionalStatusMigration'
import type { Block, Project } from '../src/lib/types'

const reference:Block=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8'))

test('saved conventional graph gains CDL status outputs while preserving tuned controls',()=>{
  const old=structuredClone(reference)
  old.inputs=old.inputs?.filter(port=>!port.name.endsWith('_occupied'))
  old.outputs=old.outputs?.filter(port=>port.name!=='preconditioning_mode'&&!port.name.endsWith('_occupancy_confirmed'))
  old.blocks=old.blocks?.filter(block=>!['conventional_preconditioning_off','conventional_occupancy_false'].includes(block.name)&&!block.name.endsWith('_occupancy_gate'))
  old.connections=old.connections?.filter(edge=>!edge.to?.includes('occupancy_gate')&&!edge.to?.endsWith('_occupancy_confirmed')&&edge.to!=='preconditioning_mode')
  const fan=old.blocks?.find(block=>block.name==='ahu_1')?.blocks?.find(block=>block.name==='fan_gain')
  if(fan)fan.parameters![0].value=0.17
  const saved={version:1,name:old.name,graph:old,scenario:{} as Project['scenario'],layout:{}} as Project
  assert.equal(needsConventionalStatusMigration(saved),true)
  const upgraded=migrateConventionalStatus(saved,reference)
  assert.equal(upgraded.graph.outputs?.filter(port=>port.name.endsWith('_occupancy_confirmed')).length,10)
  assert.equal(upgraded.graph.outputs?.find(port=>port.name==='preconditioning_mode')?.type,'Boolean')
  assert.equal(upgraded.graph.inputs?.find(port=>port.name==='zone_01_occupied')?.type,'Boolean')
  assert.equal(upgraded.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.find(block=>block.name==='fan_gain')?.parameters?.[0].value,0.17)
  assert.equal(needsConventionalStatusMigration(upgraded),false)
  assert.equal(migrateConventionalStatus(upgraded,reference),upgraded)
})
