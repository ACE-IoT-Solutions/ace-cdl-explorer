import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36GroupStatus, needsG36GroupStatusMigration } from '../src/lib/g36GroupStatusMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const previous: Record<string, string> = {
  'uGroOcc': 'group_occupied.y', 'nexOcc': 'seconds_until_occupied',
  'yCooTim': 'optimal_cooldown_longest_5.y', 'yHigOccCoo': 'cooldown_status_any_5.y',
  'yWarTim': 'optimal_warmup_longest_5.y', 'yOccHeaHig': 'warmup_status_any_5.y',
  'yOpeWin': 'mode_open_sum_5.y', 'yColZon': 'mode_cold_sum_5.y',
  'yHotZon': 'mode_hot_sum_5.y', 'ySetBac': 'average_cold.y',
  'yEndSetBac': 'all_warm_5.y', 'TZonMin': 'min_room_5.y',
  'ySetUp': 'average_hot.y', 'yEndSetUp': 'all_cool_5.y',
}

test('saved G36 groups gain pinned GroupStatus without losing tuned recovery rates', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => block.name !== 'obc_group_status')
    group.connections = group.connections!.flatMap(edge => {
      const [from, to] = edgeEndpoints(edge)
      if (to.startsWith('obc_group_status.')) return []
      if (from.startsWith('obc_group_status.')) {
        const source = previous[from.split('.')[1]]
        assert.ok(source, from)
        return [{ from: source, to }]
      }
      return [edge]
    })
  }
  const group = old.blocks!.find(block => block.name === 'zone_group_1')!
  group.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 9000
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: { zone_group_1: { x: 91, y: 22 } } }
  assert.ok(needsG36GroupStatusMigration(saved))
  const next = migrateG36GroupStatus(saved, reference)
  assert.equal(needsG36GroupStatusMigration(next), false)
  assert.deepEqual(next.layout.zone_group_1, { x: 91, y: 22 })
  for (const name of ['zone_group_1', 'zone_group_2']) {
    const migrated = next.graph.blocks!.find(block => block.name === name)!
    const expected = reference.blocks!.find(block => block.name === name)!
    assert.equal(migrated.blocks!.filter(block => block.name === 'obc_group_status').length, 1)
    for (const edge of expected.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!from.startsWith('obc_group_status.') && !to.startsWith('obc_group_status.')) continue
      assert.ok(migrated.connections!.some(current => edgeEndpoints(current).join('→') === `${from}→${to}`), `${name}: ${from}→${to}`)
    }
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value, 9000)
  assert.equal(migrateG36GroupStatus(next, reference), next)
})
