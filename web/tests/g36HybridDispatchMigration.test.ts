import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36HybridDispatch } from '../src/lib/g36HybridDispatchMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const conventional = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains capacity allocation and preserves its former dispatch preference', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const basePlant = conventional.blocks!.find(block => block.name === 'plant')!
  const newNames = new Set([
    'heat_pump_hot_eligible', 'heat_pump_chilled_eligible',
  'heat_pump_utilization_limit', 'heat_pump_zero_capacity',
  'heat_pump_capacity_nonnegative', 'heat_pump_capacity_limited', 'heat_pump_total_load', 'heat_pump_divisor_floor',
    'heat_pump_safe_total', 'heat_pump_capacity_ratio', 'heat_pump_full_fraction',
    'heat_pump_allocated_fraction', 'boiler_residual', 'chiller_residual',
    'heat_pump_mode_threshold', 'heat_pump_heating_requested', 'heat_pump_cooling_requested',
    'heat_pump_heating_mode_code', 'heat_pump_cooling_mode_code',
    'heat_pump_mode_cooling_weight', 'heat_pump_cooling_mode_weight', 'heat_pump_mode_sum',
  ])
  const dispatchTargets = new Set([
    'heat_pump_heating.u1', 'heat_pump_heating.u2',
    'heat_pump_cooling.u1', 'heat_pump_cooling.u2',
    'boiler.u1', 'boiler.u2', 'chiller.u1', 'chiller.u2',
  ])
  plant.blocks = plant.blocks!.filter(block => !newNames.has(block.name))
  plant.blocks.push(...basePlant.blocks!.filter(block =>
    ['heat_pump_dispatch_fraction', 'heat_pump_dispatch_limit', 'backup_fraction'].includes(block.name)).map(value => structuredClone(value)))
  plant.blocks.find(block => block.name === 'heat_pump_dispatch_fraction')!.parameters![0].value = 0.35
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !newNames.has(from.split('.')[0]) && !newNames.has(to.split('.')[0])
      && !dispatchTargets.has(to) && to !== 'heat_pump_requested_mode'
  })
  plant.connections.push(...basePlant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return ['heat_pump_dispatch_fraction', 'heat_pump_dispatch_limit', 'backup_fraction'].some(name =>
      from.startsWith(`${name}.`) || to.startsWith(`${name}.`)) || dispatchTargets.has(to)
  }).map(value => structuredClone(value)))
  old.inputs = old.inputs!.filter(port => port.name !== 'heat_pump_available_capacity_kw')
  plant.inputs = plant.inputs!.filter(port => port.name !== 'heat_pump_available_capacity_kw')
  old.outputs = old.outputs!.filter(port => port.name !== 'plant_heat_pump_requested_mode')
  plant.outputs = plant.outputs!.filter(port => port.name !== 'heat_pump_requested_mode')
  old.connections = old.connections!.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint.includes('heat_pump_available_capacity_kw')
      || endpoint.includes('heat_pump_requested_mode')))
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: { ...defaultScenario }, layout: { plant: { x: 42, y: 24 } } }
  const migrated = migrateG36HybridDispatch(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.35)
  assert.ok(!next.blocks!.some(block => block.name === 'heat_pump_dispatch_fraction'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'boiler_residual.y→boiler.u1'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'heat_pump_allocated_fraction.y→heat_pump_cooling.u2'))
  assert.ok(migrated.graph.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'heat_pump_available_capacity_kw→plant.heat_pump_available_capacity_kw'))
  assert.deepEqual(migrated.layout.plant, { x: 42, y: 24 })
  assert.equal(migrateG36HybridDispatch(migrated, reference), migrated)
})
