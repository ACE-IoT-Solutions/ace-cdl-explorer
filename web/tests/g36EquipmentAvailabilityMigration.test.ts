import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36EquipmentAvailability } from '../src/lib/g36EquipmentAvailabilityMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const gates = new Set(['heat_pump_availability_gate', 'boiler_availability_gate', 'chiller_availability_gate'])

test('saved G36 plant gains equipment status gates while preserving dispatch tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  plant.blocks = plant.blocks!.filter(block => !gates.has(block.name))
  plant.blocks.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value = 0.4
  plant.inputs = plant.inputs!.filter(port => !port.name.endsWith('_available'))
  old.inputs = old.inputs!.filter(port => !port.name.endsWith('_available'))
  old.connections = old.connections!.filter(edge => !edgeEndpoints(edge)[0].endsWith('_available'))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !gates.has(from.split('.')[0]) && !gates.has(to.split('.')[0])
  })
  plant.connections.push(...[
    ['heat_pump_capacity_limited.y', 'heat_pump_capacity_ratio.u1'],
    ['boiler_flow_gate.y', 'boiler_kw'], ['chiller_flow_gate.y', 'chiller_kw'],
    ['boiler_flow_gate.y', 'hot_stage_backup_active.u1'],
    ['chiller_flow_gate.y', 'chilled_stage_backup_active.u1'],
    ['heat_pump_hot_eligible.y', 'heat_pump_heating_requested.u1'],
    ['heat_pump_chilled_eligible.y', 'heat_pump_cooling_requested.u1'],
  ].map(([from, to]) => ({ from, to })))
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { plant: { x: 42, y: 24 } } }
  const migrated = migrateG36EquipmentAvailability(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.4)
  assert.equal(next.blocks!.filter(block => gates.has(block.name)).length, 3)
  for (const name of ['heat_pump', 'boiler', 'chiller']) {
    assert.ok(migrated.graph.connections!.some(edge => edgeEndpoints(edge).join('→') === `${name}_available→plant.${name}_available`))
  }
  for (const target of ['heat_pump_capacity_ratio.u1', 'boiler_kw', 'chiller_kw',
    'hot_stage_backup_active.u1', 'chilled_stage_backup_active.u1']) {
    assert.equal(next.connections!.filter(edge => edgeEndpoints(edge)[1] === target).length, 1, target)
  }
  assert.deepEqual(migrated.layout.plant, { x: 42, y: 24 })
  assert.equal(migrateG36EquipmentAvailability(migrated, reference), migrated)
})
