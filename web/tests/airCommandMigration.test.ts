import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { migrateOutdoorAirCommand } from '../src/lib/airCommandMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const graph = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block

test('older conventional projects gain editable outdoor-air commands without losing edits', () => {
  const old = structuredClone(graph)
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_outdoor_air_fraction'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_outdoor_air_fraction'))
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.outputs = ahu.outputs?.filter(port => port.name !== 'outdoor_air_fraction')
    ahu.connections = ahu.connections?.filter(edge => edgeEndpoints(edge)[1] !== 'outdoor_air_fraction')
    ahu.blocks = ahu.blocks?.filter(block => block.name !== 'minimum_outdoor_air')
  }
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_airflow_request'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_airflow_request'))
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.outputs = zone.outputs?.filter(port => port.name !== 'airflow_request')
    zone.connections = zone.connections?.filter(edge => !edgeEndpoints(edge)[0].startsWith('airflow_request_sum.') && !edgeEndpoints(edge)[1].startsWith('airflow_request_sum.') && edgeEndpoints(edge)[1] !== 'airflow_request')
    zone.blocks = zone.blocks?.filter(block => block.name !== 'airflow_request_sum')
  }
  const edited = old.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')?.parameters?.[0]
  assert.ok(edited)
  edited.value = 0.1
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { zone_ua_kw_per_k: 0.12, zone_capacity_kwh_per_k: 1, heat_pump_fraction: 0.5, heat_pump_capacity_kw: 100, boiler_capacity_kw: 100, chiller_capacity_kw: 100, step_seconds: 900 }, layout: layoutFor(old) }
  const migrated = migrateOutdoorAirCommand(saved, graph)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')?.parameters?.[0]?.value, 0.1)
  for (const ahu of [1, 2]) {
    const controller = migrated.graph.blocks?.find(block => block.name === `ahu_${ahu}`)
    assert.equal(controller?.blocks?.find(block => block.name === 'minimum_outdoor_air')?.parameters?.[0]?.value, 0.2)
    assert.equal(controller?.connections?.filter(edge => edgeEndpoints(edge)[1] === 'outdoor_air_fraction').length, 1)
    assert.equal(migrated.graph.outputs?.filter(port => port.name === `ahu_${ahu}_outdoor_air_fraction`).length, 1)
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = migrated.graph.blocks?.find(block => block.name === name)
    assert.ok(zone?.blocks?.some(block => block.name === 'airflow_request_sum'))
    assert.equal(zone?.connections?.filter(edge => edgeEndpoints(edge)[1] === 'airflow_request').length, 1)
    assert.equal(migrated.graph.outputs?.filter(port => port.name === `${name}_airflow_request`).length, 1)
  }
  assert.equal(migrateOutdoorAirCommand(migrated, graph), migrated)
})
