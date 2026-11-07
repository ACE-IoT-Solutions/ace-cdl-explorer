import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OutdoorAirLimits } from '../src/lib/g36OutdoorAirLimitsMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain occupied minimum-air gating and damper travel without losing tuning', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_minimum_outdoor_air_enabled'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end.endsWith('.occupied_mode') || end.includes('minimum_outdoor_air_enabled')))
  const newNames = new Set([
    'minimum_oa_mode_enabled', 'minimum_oa_off_setpoint', 'minimum_oa_setpoint',
    'minimum_oa_off_command', 'minimum_oa_active', 'outdoor_air_fan_off',
    'outdoor_air_fan_requested', 'outdoor_damper_physical_max', 'outdoor_damper_physical_limit',
  ])
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.inputs = ahu.inputs?.filter(port => port.name !== 'occupied_mode')
    ahu.outputs = ahu.outputs?.filter(port => port.name !== 'minimum_outdoor_air_enabled')
    ahu.blocks = ahu.blocks?.filter(block => !newNames.has(block.name))
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !newNames.has(from.split('.')[0]) && !newNames.has(to.split('.')[0])
        && from !== 'occupied_mode' && to !== 'minimum_outdoor_air_enabled'
    })
    ahu.connections?.push(
      { from: 'minimum_oa_flow_gain.y', to: 'minimum_outdoor_airflow' },
      { from: 'minimum_oa_flow_gain.y', to: 'minimum_oa_pi.u_s' },
      { from: 'minimum_oa_pi.y', to: 'minimum_oa_command.u2' },
      { from: 'minimum_oa_command.y', to: 'freeze_stage_one_outdoor_air.u2' },
      { from: 'freeze_stage_three_outdoor_air.y', to: 'outdoor_air_fraction' },
    )
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'minimum_oa_pi')!.parameters![0].value = 0.37
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 13, y: 27 } } }
  const migrated = migrateG36OutdoorAirLimits(saved, reference)
  const ahu = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  assert.equal(ahu.blocks?.find(block => block.name === 'minimum_oa_pi')?.parameters?.[0].value, 0.37)
  assert.ok(ahu.blocks?.some(block => block.name === 'outdoor_damper_physical_limit'))
  assert.equal(ahu.connections?.filter(edge => edgeEndpoints(edge)[1] === 'outdoor_air_fraction').length, 1)
  assert.ok(!ahu.connections?.some(edge => edgeEndpoints(edge)[1] === 'outdoor_air_damper_position'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'minimum_oa_setpoint.y→minimum_outdoor_airflow'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'schedule_occupied→ahu_1.occupied_mode'))
  assert.deepEqual(migrated.layout.ahu_1, { x: 13, y: 27 })
  assert.equal(migrateG36OutdoorAirLimits(migrated, reference), migrated)
})
