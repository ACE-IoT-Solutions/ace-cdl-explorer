import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalDischarge } from '../src/lib/g36TerminalDischargeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved VAVs gain discharge feedback while keeping edited setpoints and zone notes', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_discharge_air_setpoint_c'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_discharge_air_setpoint_c'))
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.outputs = zone.outputs?.filter(port => port.name !== 'discharge_air_setpoint_c')
    zone.blocks = zone.blocks?.filter(block => (!block.name.startsWith('discharge_') || block.name === 'discharge_headroom') && !block.name.startsWith('heating_airflow_') && !block.name.startsWith('heating_warmup_'))
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('discharge_') && !to.startsWith('discharge_') && !from.startsWith('heating_airflow_') && !to.startsWith('heating_airflow_') && !from.startsWith('heating_warmup_') && !to.startsWith('heating_warmup_') && to !== 'discharge_air_setpoint_c'
    })
    zone.connections?.push({ from: 'reheat_select.y', to: 'reheat_capped.u1' })
    zone.connections?.push({ from: 'heat_pi.y', to: 'heating_damper_gain.u' })
  }
  const zone = old.blocks?.find(block => block.name === 'zone_01')!
  zone.notes = 'Keep my zone note'
  zone.blocks!.find(block => block.name === 'maximum_discharge_temp')!.parameters![0].value = 33
  zone.blocks!.find(block => block.name === 'heat_pi')!.parameters![0].value = 0.7
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { zone_01: { x: 123, y: 456 } } }
  const migrated = migrateG36TerminalDischarge(saved, reference)
  const updated = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  assert.equal(updated.notes, 'Keep my zone note')
  assert.equal(updated.blocks?.find(block => block.name === 'maximum_discharge_temp')?.parameters?.[0].value, 33)
  assert.equal(updated.blocks?.find(block => block.name === 'heat_pi')?.parameters?.[0].value, 0.7)
  assert.ok(updated.blocks?.some(block => block.name === 'discharge_valve_pi'))
  assert.ok(updated.connections?.some(edge => edgeEndpoints(edge).join('→') === 'discharge_valve_select.y→reheat_capped.u1'))
  assert.ok(updated.connections?.some(edge => edgeEndpoints(edge).join('→') === 'heat_pi.y→heating_damper_gain.u'))
  assert.ok(migrated.graph.outputs?.some(port => port.name === 'zone_01_discharge_air_setpoint_c'))
  assert.deepEqual(migrated.layout.zone_01, { x: 123, y: 456 })
  assert.equal(migrateG36TerminalDischarge(migrated, reference), migrated)
})
