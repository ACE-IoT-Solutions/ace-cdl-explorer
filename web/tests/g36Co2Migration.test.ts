import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Co2, needsG36Co2Migration } from '../src/lib/g36Co2Migration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const co2Blocks = new Set([
  'co2_reset_lower', 'co2_reset_span', 'co2_above_lower', 'co2_reset_ratio',
  'co2_reset_limit', 'co2_cooling_max_flow', 'co2_available_flow',
  'co2_flow_increment', 'co2_flow_reset', 'co2_minimum_flow',
])
const hasWire = (block: Block, from: string, to: string) =>
  block.connections?.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)

test('saved G36 programs gain CO₂ sensing and CDL flow reset while retaining tuning', () => {
  const legacy = structuredClone(reference)
  legacy.inputs = legacy.inputs?.filter(port => !port.name.endsWith('_co2_ppm'))
  legacy.connections = legacy.connections?.filter(edge => !edgeEndpoints(edge)[0].endsWith('_co2_ppm'))
  legacy.brick_model!.entities = legacy.brick_model!.entities.filter(entity => !entity.id.endsWith('_co2_ppm'))
  legacy.brick_model!.relationships = legacy.brick_model!.relationships.filter(edge => !edge.object.endsWith('_co2_ppm'))
  for (const zone of legacy.blocks?.filter(block => block.name.startsWith('zone_')) ?? []) {
    zone.inputs = zone.inputs?.filter(port => port.name !== 'co2_ppm')
    zone.blocks = zone.blocks?.filter(block => !co2Blocks.has(block.name))
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !co2Blocks.has(from.split('.')[0]) && !co2Blocks.has(to.split('.')[0])
    })
    zone.connections?.push({ from: 'minimum_airflow_heating.y', to: 'airflow_sum.u1' },
      { from: 'minimum_airflow_heating.y', to: 'airflow_request_sum.u1' })
  }
  const zone = legacy.blocks!.find(block => block.name === 'zone_01')!
  zone.blocks!.find(block => block.name === 'minimum_airflow')!.parameters![0].value = 0.28
  const project: Project = { version: 1, name: legacy.name, graph: legacy, scenario: {} as Project['scenario'], layout: layoutFor(legacy) }
  assert.equal(needsG36Co2Migration(project), true)
  const migrated = migrateG36Co2(project, reference)
  assert.equal(needsG36Co2Migration(migrated), false)
  const migratedZone = migrated.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(migratedZone.blocks!.find(block => block.name === 'minimum_airflow')!.parameters![0].value, 0.28)
  assert.equal(hasWire(migratedZone, 'co2_minimum_flow.y', 'airflow_request_sum.u1'), true)
  assert.equal(hasWire(migratedZone, 'minimum_airflow_heating.y', 'airflow_request_sum.u1'), false)
  assert.equal(migrated.graph.brick_model?.entities.filter(entity => entity.class === 'brick:Zone_CO2_Level_Sensor').length, 10)
  assert.equal(migrateG36Co2(migrated, reference), migrated)
})
