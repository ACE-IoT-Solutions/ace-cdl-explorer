import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'
import { migrateG36RecoveryRate, needsG36RecoveryRateMigration } from '../src/lib/g36RecoveryRateMigration'
import { migrateG36OptimalStart } from '../src/lib/g36OptimalStartMigration'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const wire = (from: string, to: string) => ({ from, to })

test('saved optimal start learners accept slower recovery without replacing custom tuning', () => {
  const graph = structuredClone(reference)
  const group = graph.blocks!.find(block => block.name === 'zone_group_1')!
  const learner = group.blocks!.find(block => block.name === 'optimal_warmup_1_learner')!
  learner.blocks!.find(block => block.name === 'rate_mean')!.parameters![0].value = 5
  learner.blocks!.push({ name: 'rate_floor', block_type: 'Max', category: 'elementary',
    inputs: [{ name: 'u1', type: 'Real', unit: 'K/s' }, { name: 'u2', type: 'Real', unit: 'K/s' }],
    outputs: [{ name: 'y', type: 'Real', unit: 'K/s' }] })
  learner.connections = learner.connections!.filter(edge => edgeEndpoints(edge)[1] !== 'rate')
  learner.connections.push(wire('rate_mean.y', 'rate_floor.u1'), wire('fallback_rate', 'rate_floor.u2'), wire('rate_floor.y', 'rate'))
  const selector = group.blocks!.find(block => block.name === 'optimal_warmup_1_rate_safe')!
  selector.block_type = 'Max'
  selector.inputs = [{ name: 'u1', type: 'Real', unit: 'K/s' }, { name: 'u2', type: 'Real', unit: 'K/s' }]
  group.blocks = group.blocks!.filter(block => !['optimal_warmup_1_rate_available', 'optimal_zero_rate'].includes(block.name))
  group.connections = group.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !from.startsWith('optimal_zero_rate.') && !from.startsWith('optimal_warmup_1_rate_available.') &&
      !to.startsWith('optimal_warmup_1_rate_available.') && !to.startsWith('optimal_warmup_1_rate_safe.')
  })
  group.connections.push(wire('optimal_warmup_1_rate_previous.y', 'optimal_warmup_1_rate_safe.u1'),
    wire('optimal_warmup_fallback_rate.y', 'optimal_warmup_1_rate_safe.u2'))
  const saved: Project = { version: 1, name: graph.name, graph, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36RecoveryRateMigration(saved))
  const migrated = migrateG36RecoveryRate(saved, reference)
  assert.equal(needsG36RecoveryRateMigration(migrated), false)
  const nextGroup = migrated.graph.blocks!.find(block => block.name === 'zone_group_1')!
  const nextLearner = nextGroup.blocks!.find(block => block.name === learner.name)!
  assert.equal(nextLearner.blocks!.find(block => block.name === 'rate_mean')!.parameters![0].value, 5)
  assert.equal(nextLearner.blocks!.some(block => block.name === 'rate_floor'), false)
  assert.ok(nextLearner.connections!.some(edge => edgeEndpoints(edge).join('→') === 'rate_mean.y→rate'))
  assert.equal(nextGroup.blocks!.find(block => block.name === selector.name)!.block_type.split('.').pop(), 'Switch')
  assert.ok(nextGroup.blocks!.some(block => block.name === 'optimal_zero_rate'))
  assert.equal(migrateG36RecoveryRate(migrated, reference), migrated)

  const older = structuredClone(saved)
  const olderLearner = older.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === learner.name)!
  olderLearner.blocks = olderLearner.blocks!.filter(block => !['not_timed_out', 'reached_before_timeout'].includes(block.name))
  olderLearner.connections = olderLearner.connections!.filter(edge =>
    !edgeEndpoints(edge).some(end => ['not_timed_out', 'reached_before_timeout'].includes(end.split('.')[0])))
  const combined = migrateG36RecoveryRate(migrateG36OptimalStart(older, reference), reference)
  const combinedGroup = combined.graph.blocks!.find(block => block.name === 'zone_group_1')!
  const selectedInputs = combinedGroup.connections!.filter(edge => edgeEndpoints(edge)[1].startsWith(`${selector.name}.`))
  assert.equal(selectedInputs.length, 3)
  assert.equal(combinedGroup.blocks!.filter(block => block.name === 'optimal_warmup_1_rate_available').length, 1)
  assert.equal(needsG36RecoveryRateMigration(combined), false)
})
