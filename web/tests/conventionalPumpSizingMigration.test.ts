import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateConventionalPumpSizing, needsConventionalPumpSizingMigration } from '../src/lib/conventionalPumpSizingMigration'
import { defaultScenario, type Block, type Project } from '../src/lib/types'

const graph=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8')) as Block
const project=():Project=>({version:1,name:graph.name,graph:structuredClone(graph),scenario:structuredClone(defaultScenario),layout:{plant:{x:111,y:222}}})
const gain=(value:Project):Block=>{
  const found=value.graph.blocks?.find(block=>block.name==='plant')?.blocks?.find(block=>block.name==='hot_water_pump_gain')
  assert.ok(found)
  return found
}

test('stock conventional pump gain follows the branch-service design load',()=>{
  assert.equal(gain(project()).parameters?.find(parameter=>parameter.name==='k')?.value,1/90)
})

test('saved stock pump gain upgrades while custom tuning and layout survive',()=>{
  const saved=project()
  gain(saved).parameters![0].value=1/180
  gain(saved).notes='Sizes hot-water pump speed from requested load using the 180 kW design flow.'
  saved.graph.blocks!.find(block=>block.name==='plant')!.blocks!.find(block=>block.name==='chilled_water_pump_gain')!.parameters![0].value=0.031
  assert.equal(needsConventionalPumpSizingMigration(saved),true)
  const updated=migrateConventionalPumpSizing(saved)
  assert.equal(gain(updated).parameters![0].value,1/90)
  assert.equal(gain(updated).notes,gain(project()).notes)
  assert.equal(updated.graph.blocks!.find(block=>block.name==='plant')!.blocks!.find(block=>block.name==='chilled_water_pump_gain')!.parameters![0].value,0.031)
  assert.deepEqual(updated.layout.plant,{x:111,y:222})
  assert.equal(gain(saved).parameters![0].value,1/180)
  assert.equal(migrateConventionalPumpSizing(updated),updated)
  const custom=project()
  gain(custom).parameters![0].value=0.009
  assert.equal(migrateConventionalPumpSizing(custom),custom)
})
