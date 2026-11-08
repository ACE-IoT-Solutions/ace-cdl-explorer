import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36VentilationStandby, needsG36VentilationStandbyMigration } from '../src/lib/g36VentilationStandbyMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set([
  'ventilation_standby_permitted', 'ventilation_standby_disabled',
  'ventilation_area_presence_or_required', 'ventilation_area_mode_eligible',
  'ventilation_area_window_eligible',
])

test('saved G36 VAVs gain editable occupied-standby ventilation without losing custom wiring', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.blocks = zone.blocks?.filter(block => !added.has(block.name))
    zone.connections = zone.connections?.filter(edge =>
      !edgeEndpoints(edge).some(end => added.has(end.split('.')[0])))
    zone.connections?.push(
      { from: 'ventilation_window_eligible.y', to: 'ventilation_area_active.u3' },
      { from: 'window_closed', to: 'window_primary_minimum.u3' },
      ...['occupied_setpoint_requested.u1', 'mode_select_occupied.u2',
        'minimum_damper_standby.u3', 'minimum_airflow_standby.u3']
        .map(to => ({ from: 'occupancy_latch.y', to })),
    )
  }
  const zone1 = old.blocks!.find(block => block.name === 'zone_01')!
  zone1.blocks!.find(block => block.name === 'ventilation_area_rate')!.parameters![0].value = 0.0005
  const zone2 = old.blocks!.find(block => block.name === 'zone_02')!
  zone2.connections!.find(edge => edgeEndpoints(edge)[1] === 'ventilation_area_active.u3')!.from = 'ventilation_occupied.y'
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { zone_01: { x: 21, y: 34 } } }
  assert.equal(needsG36VentilationStandbyMigration(saved), true)
  const migrated = migrateG36VentilationStandby(saved, reference)
  assert.equal(needsG36VentilationStandbyMigration(migrated), false)
  assert.equal(migrated.graph.blocks?.filter(zone => zone.block_type === 'VAVBoxController' &&
    zone.blocks?.some(block => block.name === 'ventilation_standby_permitted')).length, 10)
  const zone = migrated.graph.blocks!.find(block => block.name === 'zone_01')!
  const has = (from: string, to: string) => zone.connections?.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)
  assert.equal(zone.blocks!.find(block => block.name === 'ventilation_area_rate')!.parameters![0].value, 0.0005)
  assert.ok(has('ventilation_area_window_eligible.y', 'ventilation_area_active.u3'))
  assert.ok(has('ventilation_area_window_eligible.y', 'window_primary_minimum.u3'))
  assert.ok(has('ventilation_standby_permitted.y', 'ventilation_standby_disabled.u'))
  assert.ok(has('ventilation_area_presence_or_required.y', 'minimum_airflow_standby.u3'))
  assert.ok(has('ventilation_area_presence_or_required.y', 'occupied_setpoint_requested.u1'))
  assert.ok(migrated.graph.blocks!.find(block => block.name === 'zone_02')!.connections?.some(edge =>
    edgeEndpoints(edge).join('→') === 'ventilation_occupied.y→ventilation_area_active.u3'))
  assert.deepEqual(migrated.layout.zone_01, saved.layout.zone_01)
  assert.equal(migrateG36VentilationStandby(migrated, reference), migrated)
})
