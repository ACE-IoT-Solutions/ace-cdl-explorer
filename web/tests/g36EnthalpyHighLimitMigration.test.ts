import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36EnthalpyHighLimit, needsG36EnthalpyHighLimitMigration } from '../src/lib/g36EnthalpyHighLimitMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(['economizer_enthalpy_cutoff', 'economizer_enthalpy_error',
  'economizer_enthalpy_hysteresis', 'economizer_enthalpy_and_dry_bulb_trip',
  'economizer_use_enthalpy_device', 'economizer_high_limit_device_select'])

test('saved G36 AHUs gain optional enthalpy high limit while retaining earlier device and tuning', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => port.name !== 'outdoor_air_enthalpy_kj_kg')
  old.connections = old.connections!.filter(edge => !edgeEndpoints(edge).some(end => end.includes('outdoor_air_enthalpy_kj_kg')))
  old.brick_model!.entities = old.brick_model!.entities.filter(entity => entity.id !== 'outdoor_air_enthalpy_kj_kg')
  old.brick_model!.relationships = old.brick_model!.relationships.filter(relation => relation.object !== 'outdoor_air_enthalpy_kj_kg')
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.inputs = ahu.inputs!.filter(port => port.name !== 'outdoor_air_enthalpy_kj_kg')
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => !edgeEndpoints(edge).some(end =>
      added.has(end.split('.')[0]) || end === 'outdoor_air_enthalpy_kj_kg'))
    for (const target of ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
      'economizer_high_limit_clear.u'])
      ahu.connections.push({ from: 'economizer_dry_bulb_device_select.y', to: target })
  }
  const ahu = old.blocks!.find(block => block.name === 'ahu_1')!
  ahu.blocks!.find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 41, y: 59 } } }
  assert.ok(needsG36EnthalpyHighLimitMigration(saved))
  const next = migrateG36EnthalpyHighLimit(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 41, y: 59 })
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value, 22)
  assert.ok(next.graph.inputs!.some(port => port.name === 'outdoor_air_enthalpy_kj_kg'))
  assert.ok(next.graph.brick_model!.entities.some(entity => entity.id === 'outdoor_air_enthalpy_kj_kg'))
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.ok(ahu.blocks!.some(block => block.name === 'economizer_high_limit_device_select'))
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_dry_bulb_device_select.y→economizer_high_limit_device_select.u3'))
    for (const target of ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
      'economizer_high_limit_clear.u'])
      assert.deepEqual(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
        .map(edge => edgeEndpoints(edge)[0]), ['economizer_high_limit_device_select.y'])
  }
  assert.equal(needsG36EnthalpyHighLimitMigration(next), false)
  assert.equal(migrateG36EnthalpyHighLimit(next, reference), next)

  const customized = structuredClone(saved)
  const customAhu = customized.graph.blocks!.find(block => block.name === 'ahu_1')!
  customAhu.connections!.find(edge => edgeEndpoints(edge)[1] ===
    'economizer_high_limit_sustained.u')!.from = 'economizer_freeze_clear.y'
  const customNext = migrateG36EnthalpyHighLimit(customized, reference)
  const migratedAhu = customNext.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.ok(migratedAhu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'economizer_freeze_clear.y→economizer_high_limit_sustained_enthalpy_select.u3'))
  assert.ok(migratedAhu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'economizer_high_limit_sustained_enthalpy_select.y→economizer_high_limit_sustained.u'))
})
