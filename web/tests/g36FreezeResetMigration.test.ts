import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FreezeReset } from '../src/lib/g36FreezeResetMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const oldNames = [
  'freeze_stage_three_reset_temperature', 'freeze_stage_three_reset_warm',
  'freeze_stage_three_stat_normal', 'freeze_stage_three_reset_safe',
  'freeze_stage_three_reset_allowed',
]

test('stock saved G36 AHUs connect operator reset directly to the freeze latch', () => {
  const graph = structuredClone(reference)
  graph.name = 'My G36 project'
  for (const ahu of graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks!.push(...oldNames.map(name => ({ name, block_type: name.endsWith('temperature') ? 'Constant' : 'And',
      parameters: name.endsWith('temperature') ? [{ name: 'k', value: 7 }] : [] })))
    ahu.connections = ahu.connections!.filter(edge => edgeEndpoints(edge).join('→') !== 'freeze_reset→freeze_stage_three_latch.clr')
    ahu.connections.push({ from: 'freeze_reset', to: 'freeze_stage_three_reset_allowed.u2' })
    ahu.connections.push({ from: 'freeze_stage_three_reset_allowed.y', to: 'freeze_stage_three_latch.clr' })
  }
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 24, y: 48 } } }
  const migrated = migrateG36FreezeReset(saved)
  assert.notEqual(migrated, saved)
  assert.deepEqual(migrated.layout, saved.layout)
  for (const ahu of migrated.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') === 'freeze_reset→freeze_stage_three_latch.clr'))
    assert.ok(oldNames.every(name => !ahu.blocks!.some(block => block.name === name)))
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'freeze_stage_three_latch.clr').length, 1)
  }
  assert.equal(migrateG36FreezeReset(migrated), migrated)
})

test('a user-tuned reset threshold is kept in a saved graph', () => {
  const graph = structuredClone(reference)
  const ahu = graph.blocks!.find(block => block.name === 'ahu_1')!
  ahu.blocks!.push(...oldNames.map(name => ({ name, block_type: 'And',
    parameters: name.endsWith('temperature') ? [{ name: 'k', value: 8 }] : [] })))
  ahu.connections = ahu.connections!.filter(edge => edgeEndpoints(edge).join('→') !== 'freeze_reset→freeze_stage_three_latch.clr')
  ahu.connections.push({ from: 'freeze_reset', to: 'freeze_stage_three_reset_allowed.u2' })
  ahu.connections.push({ from: 'freeze_stage_three_reset_allowed.y', to: 'freeze_stage_three_latch.clr' })
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: {} }
  assert.equal(migrateG36FreezeReset(saved), saved)
})
