import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterSourceProof } from '../src/lib/g36WaterSourceProofMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const proof = (name: string) => name.startsWith('hot_source_') || name.startsWith('chilled_source_') || name.endsWith('_flow_gate')

test('saved G36 plant gains measured flow proof while retaining source tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks?.find(block => block.name === 'plant')!
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_source_proven'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.endsWith('_source_proven')))
  plant.outputs = plant.outputs?.filter(port => !port.name.endsWith('_source_proven_output'))
  plant.blocks = plant.blocks?.filter(block => !proof(block.name))
  plant.connections = plant.connections?.filter(edge => !edgeEndpoints(edge).some(end => proof(end.split('.')[0]) || end.endsWith('_source_proven_output')))
  for (const [from, to] of [
    ['heat_pump_heating.y', 'heat_pump_heating_kw'], ['boiler.y', 'boiler_kw'],
    ['heat_pump_cooling.y', 'heat_pump_cooling_kw'], ['chiller.y', 'chiller_kw'],
  ]) plant.connections?.push({ from, to })
  plant.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value = 4.5
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 21, y: 42 } } }
  const migrated = migrateG36WaterSourceProof(saved, reference)
  const upgraded = migrated.graph.blocks?.find(block => block.name === 'plant')!
  assert.equal(upgraded.blocks?.find(block => block.name === 'hot_water_source_gain')?.parameters?.[0].value, 4.5)
  assert.ok(upgraded.blocks?.some(block => block.name === 'hot_source_proven'))
  assert.ok(upgraded.blocks?.some(block => block.name === 'chilled_source_proven'))
  assert.ok(upgraded.connections?.some(edge => edgeEndpoints(edge).join('→') === 'heat_pump_heating_flow_gate.y→heat_pump_heating_kw'))
  assert.ok(!upgraded.connections?.some(edge => edgeEndpoints(edge).join('→') === 'heat_pump_heating.y→heat_pump_heating_kw'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'plant.hot_source_proven_output→plant_hot_source_proven'))
  assert.deepEqual(migrated.layout.plant, { x: 21, y: 42 })
  assert.equal(migrateG36WaterSourceProof(migrated, reference), migrated)
})
