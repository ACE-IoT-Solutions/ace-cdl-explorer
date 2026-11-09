import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AhuPlantRequests } from '../src/lib/g36AhuPlantRequestsMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains separate AHU plant requests and retains tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const isNew = (name: string) => /^ahu_(?:1|2)_(?:hot|chilled)_plant_/.test(name) ||
    /^ahu_(?:hot|chilled)_plant_/.test(name)
  plant.blocks = plant.blocks!.filter(block => !isNew(block.name))
  plant.outputs = plant.outputs!.filter(port => !isNew(port.name))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !isNew(from.split('.')[0]) && !isNew(to.split('.')[0]) && !isNew(to)
  })
  old.outputs = old.outputs!.filter(port => !isNew(port.name.replace(/^plant_/, '')))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !isNew(from.replace(/^plant\./, '')) && !isNew(to.replace(/^plant\./, ''))
  })
  plant.connections.push({from:'hot_plant_request_or_heat.y',to:'freeze_hot_water_pump_enable.u1'})
  plant.connections.push({from:'chilled_water_pump_needed.y',to:'freeze_chilled_water_pump_enable.u1'})
  for(const block of plant.blocks ?? [])if(/^ahu_(?:1|2)_(?:hot_)?(?:two|three)_request_delay$/.test(block.name))
    block.parameters!.find(parameter=>parameter.name==='delayOnInit')!.value=true
  plant.blocks!.find(block=>block.name==='hot_reset_trim')!.parameters![0].value=-0.04
  const saved:Project={version:1,name:old.name,graph:old,layout:layoutFor(old),scenario:{} as Project['scenario']}
  const migrated=migrateG36AhuPlantRequests(saved,reference)
  const upgraded=migrated.graph.blocks!.find(block=>block.name==='plant')!
  assert.equal(upgraded.blocks!.find(block=>block.name==='hot_reset_trim')!.parameters![0].value,-0.04)
  assert.equal(upgraded.blocks!.find(block=>block.name==='ahu_1_hot_two_request_delay')!.parameters!.find(parameter=>parameter.name==='delayOnInit')!.value,false)
  assert.ok(upgraded.blocks!.some(block=>block.name==='ahu_1_hot_plant_valve_hysteresis'))
  assert.equal(upgraded.connections!.filter(edge=>edgeEndpoints(edge)[1]==='freeze_hot_water_pump_enable.u1').length,1)
  assert.equal(upgraded.connections!.filter(edge=>edgeEndpoints(edge)[1]==='freeze_chilled_water_pump_enable.u1').length,1)
  assert.ok(migrated.graph.outputs!.some(port=>port.name==='plant_ahu_hot_plant_request_count_total'))
  assert.equal(migrateG36AhuPlantRequests(migrated,reference),migrated)
})
