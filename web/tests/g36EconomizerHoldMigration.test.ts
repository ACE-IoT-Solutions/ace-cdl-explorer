import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36EconomizerHold, needsG36EconomizerHoldMigration } from '../src/lib/g36EconomizerHoldMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain the editable high-limit state hold without losing edits', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => block.name !== 'economizer_high_limit_hold')
    ahu.connections = ahu.connections!.filter(edge => edgeEndpoints(edge)[0] !== 'economizer_high_limit_hold.y')
    const edge = ahu.connections!.find(item => edgeEndpoints(item)[1] === 'economizer_high_limit_hold.u')!
    edge.to = 'economizer_high_limit_ok.u'
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_high_limit_hysteresis')!.parameters![0].value = -2
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 25, y: 37 } } }
  assert.ok(needsG36EconomizerHoldMigration(saved))
  const next = migrateG36EconomizerHold(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 25, y: 37 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.equal(ahu.blocks!.filter(block => block.name === 'economizer_high_limit_hold').length, 1)
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_ok.u').length, 1)
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'economizer_high_limit_hysteresis')!.parameters![0].value, -2)
  assert.equal(needsG36EconomizerHoldMigration(next), false)
  assert.equal(migrateG36EconomizerHold(next, reference), next)
})
