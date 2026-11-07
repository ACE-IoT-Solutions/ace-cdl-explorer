import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ChilledWaterMapping } from '../src/lib/g36ChilledWaterMappingMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gets staged chilled-water temperature mapping without losing edits', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  plant.blocks = plant.blocks!.filter(block => !block.name.startsWith('chilled_reset_temperature_'))
  plant.connections = plant.connections!.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint.startsWith('chilled_reset_temperature_')))
  plant.connections.push({ from: 'chilled_reset_previous.y', to: 'chilled_reset_inverse.u2' })
  plant.blocks.find(block => block.name === 'chilled_reset_warm_delta')!.parameters![0].value = 6
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 20, y: 40 } } }
  const migrated = migrateG36ChilledWaterMapping(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'chilled_reset_warm_delta')!.parameters![0].value, 6)
  assert.ok(next.blocks!.some(block => block.name === 'chilled_reset_temperature_scale'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_temperature_scale.y→chilled_reset_inverse.u2'))
  assert.ok(!next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_previous.y→chilled_reset_inverse.u2'))
  assert.deepEqual(migrated.layout.plant, { x: 20, y: 40 })
  assert.equal(migrateG36ChilledWaterMapping(migrated, reference), migrated)
})
