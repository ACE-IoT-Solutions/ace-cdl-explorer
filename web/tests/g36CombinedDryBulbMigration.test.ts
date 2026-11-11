import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36CombinedDryBulb, needsG36CombinedDryBulbMigration } from '../src/lib/g36CombinedDryBulbMigration'
import { migrateG36FixedDryBulb } from '../src/lib/g36FixedDryBulbMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(['economizer_use_combined_dry_bulb', 'economizer_combined_dry_bulb_trip',
  'economizer_dry_bulb_device_select'])
const enthalpyAdded = new Set(['economizer_enthalpy_cutoff', 'economizer_enthalpy_error',
  'economizer_enthalpy_hysteresis', 'economizer_enthalpy_and_dry_bulb_trip',
  'economizer_use_enthalpy_device', 'economizer_high_limit_device_select'])

test('saved G36 AHUs gain combined high limit while retaining their previous device and edits', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name) && !enthalpyAdded.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) &&
        !enthalpyAdded.has(from.split('.')[0]) && !enthalpyAdded.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'economizer_dry_bulb_high_limit_select.y', to: 'economizer_high_limit_hold.u' })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value = 21
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_use_fixed_dry_bulb')!.parameters![0].value = true
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 25, y: 37 } } }
  assert.ok(needsG36CombinedDryBulbMigration(saved))
  const next = migrateG36CombinedDryBulb(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 25, y: 37 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    for (const name of added) assert.equal(ahu.blocks!.filter(block => block.name === name).length, 1)
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u').length, 1)
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_dry_bulb_high_limit_select.y→economizer_dry_bulb_device_select.u3'))
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value, 21)
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_use_fixed_dry_bulb')!.parameters![0].value, true)
  assert.equal(needsG36CombinedDryBulbMigration(next), false)
  assert.equal(migrateG36CombinedDryBulb(next, reference), next)
  const custom = structuredClone(saved)
  custom.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .find(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u')!.from = 'economizer_freeze_clear.y'
  const migrated = migrateG36CombinedDryBulb(custom, reference)
  assert.ok(migrated.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .some(edge => edgeEndpoints(edge).join('→') === 'economizer_freeze_clear.y→economizer_dry_bulb_device_select.u3'))
})

test('older differential-only projects upgrade through both device migrations', () => {
  const old = structuredClone(reference)
  const newer = new Set([...added, ...enthalpyAdded, 'economizer_fixed_dry_bulb_cutoff',
    'economizer_fixed_dry_bulb_error', 'economizer_fixed_dry_bulb_hysteresis',
    'economizer_use_fixed_dry_bulb', 'economizer_dry_bulb_high_limit_select'])
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !newer.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !newer.has(from.split('.')[0]) && !newer.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'economizer_high_limit_hysteresis.y', to: 'economizer_high_limit_hold.u' })
  }
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: {} }
  const migrated = migrateG36CombinedDryBulb(migrateG36FixedDryBulb(saved, reference), reference)
  for (const ahu of migrated.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u').length, 1)
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_high_limit_hysteresis.y→economizer_dry_bulb_high_limit_select.u3'))
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_dry_bulb_high_limit_select.y→economizer_dry_bulb_device_select.u3'))
  }
})
