import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36HotWaterReset } from '../src/lib/g36HotWaterResetMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains sampled hot-water reset and retains edited midpoint', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const added = (name: string) => name.startsWith('hot_reset_') || /^ahu_[12]_hot_/.test(name)
  plant.inputs = plant.inputs!.filter(port => !/^ahu_[12]_heating_valve$/.test(port.name))
  plant.outputs = plant.outputs!.filter(port => port.name !== 'hot_water_reset_fraction')
  plant.blocks = plant.blocks!.filter(block => !added(block.name) && !block.name.startsWith('freeze_'))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(added(from.split('.')[0]) || added(to.split('.')[0]) || from.includes('freeze_') || to.includes('freeze_') || to === 'hot_water_reset_fraction')
  })
  plant.connections.push({ from: 'hot_water_supply_setpoint.y', to: 'hot_water_supply_setpoint_c' })
  plant.connections.push({ from: 'hot_water_supply_setpoint.y', to: 'hot_water_source_error.u1' })
  old.outputs = old.outputs!.filter(port => port.name !== 'plant_hot_water_reset_fraction')
  old.connections = old.connections!.filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !/^plant.ahu_[12]_heating_valve$/.test(to) && to !== 'plant_hot_water_reset_fraction'
  })
  plant.notes = 'My edited plant notes'
  plant.blocks.find(block => block.name === 'hot_water_supply_setpoint')!.parameters![0].value = 46.0
  const saved: Project = { version: 1, name: old.name, graph: old, layout: layoutFor(old), scenario: {} as Project['scenario'] }
  const migrated = migrateG36HotWaterReset(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.notes, 'My edited plant notes')
  assert.equal(upgraded.blocks!.find(block => block.name === 'hot_water_supply_setpoint')!.parameters![0].value, 46.0)
  assert.ok(upgraded.blocks!.some(block => block.name === 'hot_reset_previous'))
  assert.ok(upgraded.connections!.some(edge => edgeEndpoints(edge).join('|') === 'hot_reset_supply_sum.y|hot_water_supply_setpoint_c'))
  assert.equal(upgraded.connections!.filter(edge => edgeEndpoints(edge)[1] === 'hot_water_source_error.u1').length, 1)
  assert.equal(migrated.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'plant.ahu_1_heating_valve').length, 1)
  assert.equal(migrateG36HotWaterReset(migrated, reference), migrated)
})
