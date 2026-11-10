import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OptionalLowLimit, needsG36OptionalLowLimitMigration } from '../src/lib/g36OptionalLowLimitMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(['economizer_use_teaching_low_limit', 'economizer_low_limit_bypass',
  'economizer_low_limit_select'])

test('saved G36 AHUs retain their low-temperature guard while new examples bypass it', () => {
  const bundled = reference.blocks!.find(block => block.name === 'ahu_1')!
  assert.equal(bundled.blocks!.find(block => block.name === 'economizer_use_teaching_low_limit')!.parameters![0].value, false)
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'outdoor_above_low_limit.y', to: 'economizer_temperature_ok.u1' })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_low_limit')!.parameters![0].value = 10
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    const pi = ahu.blocks!.find(block => block.name === 'economizer_mixed_air_pi')!
    pi.block_type = 'LimPID'
    pi.inputs = pi.inputs!.filter(port => port.name !== 'trigger')
    pi.parameters = pi.parameters!.filter(parameter => !['r', 'Ni', 'y_reset', 'xi_start'].includes(parameter.name))
    ahu.connections = ahu.connections!.filter(edge => edgeEndpoints(edge)[1] !== 'economizer_mixed_air_pi.trigger')
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_mixed_air_pi')!.parameters!
    .find(parameter => parameter.name === 'k')!.value = 0.07
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 25, y: 37 } } }
  assert.ok(needsG36OptionalLowLimitMigration(saved))
  const renamed = structuredClone(saved)
  renamed.name = 'My building controls'
  renamed.graph.name = renamed.name
  assert.ok(needsG36OptionalLowLimitMigration(renamed))
  assert.equal(migrateG36OptionalLowLimit(renamed, reference).graph.name, renamed.name)
  const next = migrateG36OptionalLowLimit(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 25, y: 37 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.equal(ahu.blocks!.find(block => block.name === 'economizer_use_teaching_low_limit')!.parameters![0].value, true)
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_temperature_ok.u1').length, 1)
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'outdoor_above_low_limit.y→economizer_low_limit_select.u1'))
    assert.equal(ahu.blocks!.find(block => block.name === 'economizer_mixed_air_pi')!.block_type, 'PIDWithReset')
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_enabled.y→economizer_mixed_air_pi.trigger'))
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_low_limit')!.parameters![0].value, 10)
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_mixed_air_pi')!.parameters!
    .find(parameter => parameter.name === 'k')!.value, 0.07)
  assert.equal(needsG36OptionalLowLimitMigration(next), false)
  assert.equal(migrateG36OptionalLowLimit(next, reference), next)
  const custom = structuredClone(saved)
  custom.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .find(edge => edgeEndpoints(edge)[1] === 'economizer_temperature_ok.u1')!.from = 'economizer_freeze_clear.y'
  const migrated = migrateG36OptionalLowLimit(custom, reference)
  assert.ok(migrated.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .some(edge => edgeEndpoints(edge).join('→') === 'economizer_freeze_clear.y→economizer_low_limit_select.u1'))
})
