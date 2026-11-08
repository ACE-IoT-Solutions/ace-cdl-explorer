import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ObcOptimalStart, needsG36ObcOptimalStartMigration } from '../src/lib/g36ObcOptimalStartMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const oldGraph = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block
const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 controls migrate both groups to editable OBC optimal start and preserve tuning', () => {
  const old = structuredClone(oldGraph)
  const first = old.blocks!.find(block => block.name === 'zone_group_1')!
  first.blocks!.find(block => block.name === 'maximum_start_seconds')!.parameters![0].value = 7200
  first.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 2400
  first.blocks!.find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters!.find(parameter => parameter.name === 'n')!.value = 5
  first.notes = 'Operator note'
  const saved: Project = {
    version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_group_1: { x: 91, y: 22 } },
  }
  assert.ok(needsG36ObcOptimalStartMigration(saved))
  const next = migrateG36ObcOptimalStart(saved, reference)
  assert.equal(needsG36ObcOptimalStartMigration(next), false)
  assert.deepEqual(next.layout.zone_group_1, { x: 91, y: 22 })
  assert.match(next.graph.blocks!.find(block => block.name === 'zone_group_1')!.notes!, /Operator note/)
  for (const name of ['zone_group_1', 'zone_group_2']) {
    const group = next.graph.blocks!.find(block => block.name === name)!
    const expected = reference.blocks!.find(block => block.name === name)!
    assert.equal(group.blocks!.filter(block => block.name === 'optimal_warmup_1_obc').length, 1)
    assert.equal(group.blocks!.filter(block => block.name === 'optimal_cooldown_5_obc').length, 1)
    assert.ok(group.blocks!.every(block => !block.name.endsWith('_learner')))
    const edges = new Set((group.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of expected.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!from.includes('_obc.') && !to.includes('_obc.') && !from.startsWith('optimal_start_') && !to.startsWith('optimal_start_')) continue
      assert.ok(edges.has(`${from}→${to}`), `${name}: ${from}→${to}`)
    }
    const targets = (group.connections ?? []).map(edge => edgeEndpoints(edge)[1])
    assert.equal(new Set(targets).size, targets.length, `${name} has duplicate destinations`)
  }
  const migrated = next.graph.blocks!.find(block => block.name === 'zone_group_1')!
  const warm = migrated.blocks!.find(block => block.name === 'optimal_warmup_1_obc')!
  const parameter = (name: string, key: string) => warm.blocks!.find(block => block.name === name)!
    .parameters!.find(item => item.name === key)!.value
  assert.equal(parameter('maxStaTim', 'k'), 7200)
  assert.equal(parameter('defTemSlo', 'k'), 1 / 2400)
  assert.equal(parameter('samTemSloAve', 'n'), 5)
  assert.equal(migrated.blocks!.find(block => block.name === 'optimal_start_horizon_offset')!.parameters![0].value, -7200)
  assert.equal(migrateG36ObcOptimalStart(next, reference), next)
})
