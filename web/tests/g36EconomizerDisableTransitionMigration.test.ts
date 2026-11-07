import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36EconomizerDisableTransition, needsG36EconomizerDisableTransitionMigration } from '../src/lib/g36EconomizerDisableTransitionMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const names = new Set([
  'economizer_disable', 'economizer_outdoor_close_delay', 'economizer_outdoor_transition_pending',
  'economizer_outdoor_transition_safe', 'economizer_return_release_delay',
  'economizer_return_transition_pending', 'economizer_return_transition_active',
  'economizer_return_transition_select',
])

test('saved G36 AHUs gain timed damper transitions and retain tuned blocks', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !names.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !names.has(from.split('.')[0]) && !names.has(to.split('.')[0])
    })
    ahu.connections.push(
      { from: 'economizer_enabled.y', to: 'outdoor_air_select.u3' },
      { from: 'return_damper_physical_limit.y', to: 'return_air_damper_position' },
    )
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'return_damper_physical_max')!.parameters![0].value = 0.9
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 25, y: 37 } } }
  assert.ok(needsG36EconomizerDisableTransitionMigration(saved))
  const next = migrateG36EconomizerDisableTransition(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 25, y: 37 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    for (const name of names) assert.equal(ahu.blocks!.filter(block => block.name === name).length, 1)
    for (const target of ['outdoor_air_select.u3', 'return_air_damper_position'])
      assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === target).length, 1)
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'return_damper_physical_max')!.parameters![0].value, 0.9)
  assert.equal(needsG36EconomizerDisableTransitionMigration(next), false)
  assert.equal(migrateG36EconomizerDisableTransition(next, reference), next)
})
