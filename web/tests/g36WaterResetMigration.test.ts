import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterReset } from '../src/lib/g36WaterResetMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains sampled chilled-water reset and retains edited minimum', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const newName = (name: string) => name.startsWith('chilled_reset_') || /^ahu_[12]_(valve_request|supply_error|two_request|three_request|request_two|request_three|select_two_requests|select_three_requests)/.test(name)
  plant.inputs = plant.inputs!.filter(port => !/^ahu_[12]_(cooling_valve|supply_setpoint_c|supply_actual_c)$/.test(port.name))
  plant.outputs = plant.outputs!.filter(port => port.name !== 'chilled_water_reset_fraction')
  plant.blocks = plant.blocks!.filter(block => !newName(block.name))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(newName(from.split('.')[0]) || newName(to.split('.')[0]) || to === 'chilled_water_reset_fraction')
  })
  plant.connections.push({ from: 'chilled_water_supply_setpoint.y', to: 'chilled_water_supply_setpoint_c' })
  old.outputs = old.outputs!.filter(port => port.name !== 'plant_chilled_water_reset_fraction')
  old.connections = old.connections!.filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !to.startsWith('plant.ahu_') && to !== 'plant_chilled_water_reset_fraction'
  })
  plant.notes = 'My edited plant notes'
  plant.blocks.find(block => block.name === 'chilled_water_supply_setpoint')!.parameters![0].value = 8.0
  const saved: Project = { version: 1, name: old.name, graph: old, layout: layoutFor(old), scenario: {} as Project['scenario'] }
  const migrated = migrateG36WaterReset(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.notes, 'My edited plant notes')
  assert.equal(upgraded.blocks!.find(block => block.name === 'chilled_water_supply_setpoint')!.parameters![0].value, 8.0)
  assert.ok(upgraded.blocks!.some(block => block.name === 'chilled_reset_previous'))
  assert.ok(upgraded.connections!.some(edge => edgeEndpoints(edge).join('|') === 'chilled_reset_supply_sum.y|chilled_water_supply_setpoint_c'))
  assert.equal(upgraded.connections!.filter(edge => edgeEndpoints(edge)[1] === 'chilled_water_supply_setpoint_c').length, 1)
  assert.equal(migrated.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'plant.ahu_1_cooling_valve').length, 1)
  assert.equal(migrateG36WaterReset(migrated, reference), migrated)
})
