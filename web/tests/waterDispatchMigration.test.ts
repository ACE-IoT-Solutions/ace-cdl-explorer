import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateWaterDispatch } from '../src/lib/waterDispatchMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  test(`${file} carries the scenario dispatch share into editable plant CDL`, () => {
    const reference = JSON.parse(readFileSync(new URL(`../public/examples/${file}`, import.meta.url), 'utf8')) as Block
    const old = structuredClone(reference)
    const plant = old.blocks!.find(block => block.name === 'plant')!
    old.inputs!.push({ name: 'heat_pump_fraction', type: 'Real', unit: 'fraction (0–1)' })
    old.connections!.push({ from: 'heat_pump_fraction', to: 'plant.heat_pump_fraction' })
    plant.inputs!.push({ name: 'heat_pump_fraction', type: 'Real', unit: 'fraction (0–1)' })
    plant.blocks = plant.blocks!.filter(block => !block.name.startsWith('heat_pump_dispatch_'))
    plant.connections = plant.connections!.filter(edge =>
      !edgeEndpoints(edge).some(endpoint => endpoint.startsWith('heat_pump_dispatch_')))
    for (const target of ['backup_fraction.u2', 'heat_pump_heating.u2', 'heat_pump_cooling.u2']) {
      plant.connections.push({ from: 'heat_pump_fraction', to: target })
    }
    plant.blocks.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value = 8
    const saved: Project = { version: 1, name: old.name, graph: old,
      scenario: { ...defaultScenario, heat_pump_fraction: 0.35 }, layout: { plant: { x: 18, y: 24 } } }
    const migrated = migrateWaterDispatch(saved, reference)
    const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
    assert.equal(next.blocks!.find(block => block.name === 'heat_pump_dispatch_fraction')!.parameters![0].value, 0.35)
    assert.equal(next.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value, 8)
    assert.ok(next.blocks!.some(block => block.name === 'heat_pump_dispatch_limit'))
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'backup_fraction.y→boiler.u2'))
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'heat_pump_dispatch_limit.y→backup_fraction.u2'))
    assert.ok(!migrated.graph.inputs!.some(port => port.name === 'heat_pump_fraction'))
    assert.ok(!next.inputs!.some(port => port.name === 'heat_pump_fraction'))
    assert.equal(migrated.scenario.heat_pump_fraction, undefined)
    assert.deepEqual(migrated.layout.plant, { x: 18, y: 24 })
    assert.equal(migrateWaterDispatch(migrated, reference), migrated)
  })
}

test('an already edited dispatch block takes precedence over a stale scenario share', () => {
  const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
  const graph = structuredClone(reference)
  graph.blocks!.find(block => block.name === 'plant')!.blocks!
    .find(block => block.name === 'heat_pump_dispatch_fraction')!.parameters![0].value = 0.8
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: { ...defaultScenario, heat_pump_fraction: 0.2 }, layout: {} }
  const migrated = migrateWaterDispatch(saved, reference)
  assert.equal(migrated.graph.blocks!.find(block => block.name === 'plant')!.blocks!
    .find(block => block.name === 'heat_pump_dispatch_fraction')!.parameters![0].value, 0.8)
  assert.equal(migrated.scenario.heat_pump_fraction, undefined)
})
