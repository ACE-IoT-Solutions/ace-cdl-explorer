import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WindowMinimum, needsG36WindowMinimumMigration } from '../src/lib/g36WindowMinimumMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 VAVs gain an editable open-window minimum-flow gate', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.blocks = zone.blocks?.filter(block => block.name !== 'window_primary_minimum')
    zone.connections = zone.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.startsWith('window_primary_minimum.')))
    zone.connections?.push(
      { from: 'co2_minimum_flow.y', to: 'airflow_sum.u1' },
      { from: 'co2_minimum_flow.y', to: 'airflow_request_sum.u1' },
    )
  }
  const first = old.blocks?.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'co2_reset_lower')!.parameters![0].value = 650
  const customZone = old.blocks?.find(block => block.name === 'zone_02')!
  const customEdge = customZone.connections!.find(edge => edgeEndpoints(edge).join('→') === 'co2_minimum_flow.y→airflow_request_sum.u1')!
  customEdge.from = 'co2_flow_reset.y'
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { zone_01: { x: 19, y: 23 } } }
  assert.equal(needsG36WindowMinimumMigration(saved), true)
  const migrated = migrateG36WindowMinimum(saved, reference)
  const zone = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  const has = (from: string, to: string) => zone.connections?.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)
  assert.equal(zone.blocks?.find(block => block.name === 'co2_reset_lower')?.parameters?.[0].value, 650)
  assert.ok(zone.blocks?.some(block => block.name === 'window_primary_minimum' && block.block_type === 'Switch'))
  assert.ok(has('co2_minimum_flow.y', 'window_primary_minimum.u1'))
  assert.ok(has('ventilation_zero.y', 'window_primary_minimum.u2'))
  assert.ok(has('window_closed', 'window_primary_minimum.u3'))
  assert.ok(has('window_primary_minimum.y', 'airflow_request_sum.u1'))
  assert.ok(!has('co2_minimum_flow.y', 'airflow_request_sum.u1'))
  assert.ok(migrated.graph.blocks?.find(block => block.name === 'zone_02')?.connections?.some(edge =>
    edgeEndpoints(edge).join('→') === 'co2_flow_reset.y→airflow_request_sum.u1'))
  assert.deepEqual(migrated.layout.zone_01, saved.layout.zone_01)
  assert.equal(needsG36WindowMinimumMigration(migrated), false)
  assert.equal(migrateG36WindowMinimum(migrated, reference), migrated)
})
