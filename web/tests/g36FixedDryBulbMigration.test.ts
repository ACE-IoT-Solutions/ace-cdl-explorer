import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FixedDryBulb, needsG36FixedDryBulbMigration } from '../src/lib/g36FixedDryBulbMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set([
  'economizer_fixed_dry_bulb_cutoff', 'economizer_fixed_dry_bulb_error',
  'economizer_fixed_dry_bulb_hysteresis', 'economizer_use_fixed_dry_bulb',
  'economizer_dry_bulb_high_limit_select',
])
const legacyRemoved = new Set([...added, 'economizer_use_combined_dry_bulb',
  'economizer_combined_dry_bulb_trip', 'economizer_dry_bulb_device_select',
  'economizer_enthalpy_cutoff', 'economizer_enthalpy_error',
  'economizer_enthalpy_hysteresis', 'economizer_enthalpy_and_dry_bulb_trip',
  'economizer_use_enthalpy_device', 'economizer_high_limit_device_select'])

test('saved G36 AHUs gain a selectable fixed high limit without losing differential tuning', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !legacyRemoved.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !legacyRemoved.has(from.split('.')[0]) && !legacyRemoved.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'economizer_high_limit_hysteresis.y', to: 'economizer_high_limit_hold.u' })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_high_limit_hysteresis')!.parameters![0].value = -2
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 25, y: 37 } } }
  assert.ok(needsG36FixedDryBulbMigration(saved))
  const next = migrateG36FixedDryBulb(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 25, y: 37 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    for (const name of added) assert.equal(ahu.blocks!.filter(block => block.name === name).length, 1)
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u').length, 1)
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_high_limit_hysteresis')!.parameters![0].value, -2)
  assert.equal(needsG36FixedDryBulbMigration(next), false)
  assert.equal(migrateG36FixedDryBulb(next, reference), next)

  const rewired = structuredClone(saved)
  const custom = rewired.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .find(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u')!
  custom.from = 'economizer_freeze_clear.y'
  const migratedCustom = migrateG36FixedDryBulb(rewired, reference)
  const customAhu = migratedCustom.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.ok(customAhu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'economizer_freeze_clear.y→economizer_dry_bulb_high_limit_select.u3'))
  assert.equal(customAhu.connections!.filter(edge => edgeEndpoints(edge)[1] ===
    'economizer_dry_bulb_high_limit_select.u3').length, 1)
})
