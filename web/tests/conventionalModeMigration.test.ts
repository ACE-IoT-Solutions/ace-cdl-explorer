import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateConventionalMode, needsConventionalModeMigration } from '../src/lib/conventionalModeMigration'
import type { Block, Project } from '../src/lib/types'

const reference:Block=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8'))

test('saved conventional dispatch gains CDL mode while retaining edited commands',()=>{
  const old=structuredClone(reference)
  const plant=old.blocks!.find(block=>block.name==='plant')!
  const modeBlocks=new Set(['heat_pump_mode_threshold','heat_pump_heating_requested',
    'heat_pump_cooling_requested','heat_pump_heating_mode_code','heat_pump_cooling_mode_code',
    'heat_pump_mode_cooling_weight','heat_pump_cooling_mode_weight','heat_pump_mode_sum'])
  plant.blocks=plant.blocks!.filter(block=>!modeBlocks.has(block.name))
  plant.outputs=plant.outputs!.filter(port=>port.name!=='heat_pump_requested_mode')
  plant.connections=plant.connections!.filter(edge=>!modeBlocks.has(edge.to!.split('.')[0])
    && !modeBlocks.has(edge.from!.split('.')[0]) && edge.to!=='heat_pump_requested_mode')
  old.outputs=old.outputs!.filter(port=>port.name!=='plant_heat_pump_requested_mode')
  old.connections=old.connections!.filter(edge=>edge.to!=='plant_heat_pump_requested_mode')
  plant.blocks!.find(block=>block.name==='heat_pump_dispatch_fraction')!.parameters![0].value=0.43
  const saved={version:1,name:old.name,graph:old,scenario:{} as Project['scenario'],layout:{}} as Project
  assert.equal(needsConventionalModeMigration(saved),true)
  const migrated=migrateConventionalMode(saved,reference)
  assert.equal(migrated.graph.blocks?.find(block=>block.name==='plant')?.blocks?.find(block=>block.name==='heat_pump_dispatch_fraction')?.parameters?.[0].value,0.43)
  assert.equal(migrated.graph.outputs?.find(port=>port.name==='plant_heat_pump_requested_mode')?.type,'Integer')
  assert.equal(migrated.graph.blocks?.find(block=>block.name==='plant')?.connections?.some(edge=>edge.to==='heat_pump_requested_mode'),true)
  assert.equal(needsConventionalModeMigration(migrated),false)
  assert.equal(migrateConventionalMode(migrated,reference),migrated)
})
