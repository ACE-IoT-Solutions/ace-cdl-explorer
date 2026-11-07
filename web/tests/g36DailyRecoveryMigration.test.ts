import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'
import { migrateG36DailyRecovery, needsG36DailyRecoveryMigration } from '../src/lib/g36DailyRecoveryMigration'
import { migrateG36OptimalStart } from '../src/lib/g36OptimalStartMigration'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(['latest_completed_rate', 'daily_completion', 'daily_completion_fall', 'daily_rate'])

test('saved G36 recovery learners gain midnight sampling without losing tuning', () => {
  const legacy = structuredClone(reference)
  for (const group of legacy.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => block.name !== 'optimal_daily_sample')
    group.connections = group.connections!.filter(edge => edgeEndpoints(edge)[0] !== 'optimal_daily_sample.y')
    for (const learner of group.blocks ?? []) {
      if (!/^optimal_(warmup|cooldown)_[1-5]_learner$/.test(learner.name)) continue
      learner.inputs = learner.inputs!.filter(port => port.name !== 'midnight')
      learner.blocks = learner.blocks!.filter(block => !added.has(block.name))
      learner.connections = learner.connections!.filter(edge => {
        const [from, to] = edgeEndpoints(edge)
        return from !== 'midnight' && !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
      })
      learner.connections.push({ from: 'rate_sample.y', to: 'rate_mean.u' },
        { from: 'completion_edge.y', to: 'rate_mean.trigger' })
    }
  }
  legacy.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters![0].value = 5
  const saved: Project = { version: 1, name: legacy.name, graph: legacy, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36DailyRecoveryMigration(saved))
  const migrated = migrateG36DailyRecovery(saved, reference)
  assert.equal(needsG36DailyRecoveryMigration(migrated), false)
  for (const group of migrated.graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    assert.equal(group.blocks!.filter(block => block.name === 'optimal_daily_sample').length, 1)
    for (const learner of group.blocks ?? []) {
      if (!/^optimal_(warmup|cooldown)_[1-5]_learner$/.test(learner.name)) continue
      assert.ok(learner.inputs!.some(port => port.name === 'midnight'))
      assert.equal(learner.connections!.filter(edge => edgeEndpoints(edge)[1] === 'rate_mean.trigger').length, 1)
      assert.ok(group.connections!.some(edge => edgeEndpoints(edge).join('→') === `optimal_daily_sample.y→${learner.name}.midnight`))
    }
  }
  assert.equal(migrated.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters![0].value, 5)
  assert.equal(migrateG36DailyRecovery(migrated, reference), migrated)

  const older = structuredClone(saved)
  const olderLearner = older.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'optimal_warmup_1_learner')!
  olderLearner.blocks = olderLearner.blocks!.filter(block => block.name !== 'reached_before_timeout')
  olderLearner.connections = olderLearner.connections!.filter(edge =>
    !edgeEndpoints(edge).some(end => end.startsWith('reached_before_timeout.')))
  const combined = migrateG36DailyRecovery(migrateG36OptimalStart(older, reference), reference)
  const combinedGroup = combined.graph.blocks!.find(block => block.name === 'zone_group_1')!
  assert.equal(combinedGroup.blocks!.filter(block => block.name === 'optimal_daily_sample').length, 1)
  assert.equal(combinedGroup.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_1_learner.midnight').length, 1)
  assert.equal(needsG36DailyRecoveryMigration(combined), false)
})
