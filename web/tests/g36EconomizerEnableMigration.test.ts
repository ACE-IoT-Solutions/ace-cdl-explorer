import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36EconomizerEnable } from '../src/lib/g36EconomizerEnableMigration'
import { migrateG36PressureTrim } from '../src/lib/g36Migration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 economizers gain fan proof and dry-bulb hysteresis while preserving tuning', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs?.filter(port => !port.name.includes('supply_fan_proven'))
  old.outputs = old.outputs?.filter(port => !port.name.includes('economizer_mode_enabled'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end.includes('supply_fan_proven') || end.includes('economizer_mode_enabled')))
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.inputs = ahu.inputs?.filter(port => port.name !== 'supply_fan_proven')
    ahu.outputs = ahu.outputs?.filter(port => port.name !== 'economizer_mode_enabled')
    ahu.blocks = ahu.blocks?.filter(block => !block.name.startsWith('economizer_high_limit_')
      && !block.name.startsWith('economizer_freeze_') && block.name !== 'economizer_proven_and_clear')
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return ![from, to].some(end => end.startsWith('economizer_high_limit_')
        || end.startsWith('economizer_freeze_') || end.startsWith('economizer_proven_and_clear')
        || end === 'supply_fan_proven' || end === 'economizer_mode_enabled')
    })
    ahu.connections?.push({ from: 'outdoor_below_return.y', to: 'free_cooling_useful.u1' })
    ahu.connections?.push({ from: 'fan_requested.y', to: 'economizer_temperature_ok.u2' })
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'economizer_mixed_air_pi')!.parameters![0].value = 0.23
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 19, y: 31 } } }
  const beforePressure = structuredClone(saved)
  const pressureAhu = beforePressure.graph.blocks?.find(block => block.name === 'ahu_1')!
  pressureAhu.blocks = pressureAhu.blocks?.filter(block => block.name !== 'pressure_previous')
  pressureAhu.connections = pressureAhu.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.startsWith('pressure_previous.')))
  const pressureMigrated = migrateG36PressureTrim(beforePressure, reference)
  assert.ok(!pressureMigrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks
    ?.some(block => block.name === 'economizer_high_limit_hysteresis'))
  const migrated = migrateG36EconomizerEnable(saved, reference)
  const ahu = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  assert.equal(ahu.blocks?.find(block => block.name === 'economizer_mixed_air_pi')?.parameters?.[0].value, 0.23)
  assert.ok(ahu.blocks?.some(block => block.name === 'economizer_high_limit_hysteresis'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'economizer_proven_and_clear.y→economizer_temperature_ok.u2'))
  assert.equal(ahu.connections?.filter(edge => edgeEndpoints(edge)[1] === 'economizer_temperature_ok.u2').length, 1)
  assert.ok(migrated.graph.inputs?.some(port => port.name === 'ahu_1_supply_fan_proven'))
  assert.deepEqual(migrated.layout.ahu_1, { x: 19, y: 31 })
  assert.equal(migrateG36EconomizerEnable(migrated, reference), migrated)
})
