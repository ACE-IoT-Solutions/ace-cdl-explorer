import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DifferentialEnthalpy, needsG36DifferentialEnthalpyMigration } from '../src/lib/g36DifferentialEnthalpyMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set([
  'economizer_differential_enthalpy_error', 'economizer_differential_enthalpy_hysteresis',
  'economizer_differential_enthalpy_and_dry_bulb_trip',
  'economizer_use_differential_enthalpy_device', 'economizer_differential_enthalpy_device_select',
])

test('saved G36 AHUs gain differential enthalpy without replacing tuned prior high limits', () => {
  const old = structuredClone(reference)
  for (const index of [1, 2]) {
    const point = `ahu_${index}_return_air_enthalpy_kj_kg`
    old.inputs = old.inputs!.filter(port => port.name !== point)
    old.connections = old.connections!.filter(edge => edgeEndpoints(edge)[0] !== point)
    old.brick_model!.entities = old.brick_model!.entities.filter(entity => entity.id !== point)
    old.brick_model!.relationships = old.brick_model!.relationships.filter(relation => relation.object !== point)
    const ahu = old.blocks!.find(block => block.name === `ahu_${index}`)!
    ahu.inputs = ahu.inputs!.filter(port => port.name !== 'return_air_enthalpy_kj_kg')
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => !edgeEndpoints(edge).some(end =>
      added.has(end.split('.')[0]) || end === 'return_air_enthalpy_kj_kg'))
    for (const target of ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
      'economizer_high_limit_clear.u'])
      ahu.connections.push({ from: 'economizer_high_limit_device_select.y', to: target })
  }
  const ahu = old.blocks!.find(block => block.name === 'ahu_1')!
  ahu.blocks!.find(block => block.name === 'economizer_enthalpy_cutoff')!.parameters![0].value = 62
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 41, y: 59 } } }
  assert.ok(needsG36DifferentialEnthalpyMigration(saved))
  const next = migrateG36DifferentialEnthalpy(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 41, y: 59 })
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_enthalpy_cutoff')!.parameters![0].value, 62)
  for (const index of [1, 2]) {
    const point = `ahu_${index}_return_air_enthalpy_kj_kg`
    const migrated = next.graph.blocks!.find(block => block.name === `ahu_${index}`)!
    assert.ok(next.graph.inputs!.some(port => port.name === point))
    assert.ok(next.graph.brick_model!.entities.some(entity => entity.id === point))
    assert.ok(next.graph.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      `${point}→ahu_${index}.return_air_enthalpy_kj_kg`))
    assert.ok(migrated.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_high_limit_device_select.y→economizer_differential_enthalpy_device_select.u3'))
    for (const target of ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
      'economizer_high_limit_clear.u'])
      assert.deepEqual(migrated.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
        .map(edge => edgeEndpoints(edge)[0]), ['economizer_differential_enthalpy_device_select.y'])
  }
  assert.equal(needsG36DifferentialEnthalpyMigration(next), false)
  assert.equal(migrateG36DifferentialEnthalpy(next, reference), next)

  const partial = structuredClone(next)
  const missingPoint = 'ahu_1_return_air_enthalpy_kj_kg'
  partial.graph.inputs = partial.graph.inputs!.filter(port => port.name !== missingPoint)
  partial.graph.connections = partial.graph.connections!.filter(edge => edgeEndpoints(edge)[0] !== missingPoint)
  partial.graph.brick_model!.entities = partial.graph.brick_model!.entities.filter(entity => entity.id !== missingPoint)
  partial.graph.brick_model!.relationships = partial.graph.brick_model!.relationships.filter(relation => relation.object !== missingPoint)
  partial.graph.blocks!.find(block => block.name === 'ahu_1')!.inputs =
    partial.graph.blocks!.find(block => block.name === 'ahu_1')!.inputs!
      .filter(port => port.name !== 'return_air_enthalpy_kj_kg')
  assert.ok(needsG36DifferentialEnthalpyMigration(partial))
  const repaired = migrateG36DifferentialEnthalpy(partial, reference)
  assert.equal(needsG36DifferentialEnthalpyMigration(repaired), false)
  assert.equal(repaired.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .filter(block => block.name === 'economizer_differential_enthalpy_device_select').length, 1)

  const halfSaved = structuredClone(next)
  const halfAhu = halfSaved.graph.blocks!.find(block => block.name === 'ahu_1')!
  halfAhu.blocks = halfAhu.blocks!.filter(block => block.name !== 'economizer_differential_enthalpy_device_select')
  halfAhu.connections = halfAhu.connections!.filter(edge => !edgeEndpoints(edge).some(end =>
    end.split('.')[0] === 'economizer_differential_enthalpy_device_select'))
  for (const target of ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
    'economizer_high_limit_clear.u'])
    halfAhu.connections.push({ from: 'economizer_high_limit_device_select.y', to: target })
  const resumed = migrateG36DifferentialEnthalpy(halfSaved, reference)
  const resumedAhu = resumed.graph.blocks!.find(block => block.name === 'ahu_1')!
  for (const name of added)
    assert.equal(resumedAhu.blocks!.filter(block => block.name === name).length, 1)

  const custom = structuredClone(saved)
  const customAhu = custom.graph.blocks!.find(block => block.name === 'ahu_1')!
  customAhu.connections!.find(edge => edgeEndpoints(edge)[1] ===
    'economizer_high_limit_sustained.u')!.from = 'economizer_freeze_clear.y'
  const migratedCustom = migrateG36DifferentialEnthalpy(custom, reference)
  assert.ok(migratedCustom.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_freeze_clear.y→economizer_high_limit_sustained_differential_enthalpy_select.u3'))
})
