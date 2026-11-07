import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateConventionalSourceEnable } from '../src/lib/conventionalSourceEnableMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block

test('saved conventional plant gates each source in CDL and retains tuned source gain', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  plant.blocks = plant.blocks!.filter(block => !block.name.endsWith('_enable_gate'))
  plant.connections = plant.connections!.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint.includes('_enable_gate.')))
  for (const [command, output] of [
    ['heat_pump_heating', 'heat_pump_heating_kw'], ['heat_pump_cooling', 'heat_pump_cooling_kw'],
    ['boiler', 'boiler_kw'], ['chiller', 'chiller_kw'],
  ]) plant.connections.push({ from: `${command}.y`, to: output })
  plant.blocks.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value = 9
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 25, y: 30 } } }
  const migrated = migrateConventionalSourceEnable(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value, 9)
  for (const output of ['heat_pump_heating_kw', 'heat_pump_cooling_kw', 'boiler_kw', 'chiller_kw']) {
    assert.deepEqual(next.connections!.filter(edge => edgeEndpoints(edge)[1] === output).map(edge => edgeEndpoints(edge)[0]),
      [`${output.replace('_kw', '')}_enable_gate.y`])
  }
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_water_pump_needed.y→boiler_enable_gate.u3'))
  assert.deepEqual(migrated.layout.plant, { x: 25, y: 30 })
  assert.equal(migrateConventionalSourceEnable(migrated, reference), migrated)
})
