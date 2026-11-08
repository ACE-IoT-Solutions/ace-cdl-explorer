import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OptimalStart, needsG36OptimalStartMigration } from '../src/lib/g36OptimalStartMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block
const wire = (from: string, to: string) => ({ from, to })
const identity = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')

test('saved zone groups gain per-zone learned optimal start while keeping tuned rates and setpoints', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => !block.name.startsWith('optimal_'))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.split('.')[0].startsWith('optimal_') && !to.split('.')[0].startsWith('optimal_')
    })
    for (const mode of ['warmup', 'cooldown']) {
      group.connections.push(wire(`${mode}_positive_error.y`, `${mode}_seconds_per_k.u`))
      group.connections.push(wire(`${mode}_seconds_per_k.y`, `${mode}_time_capped.u1`))
    }
  }
  const first = old.blocks!.find(block => block.name === 'zone_group_1')!
  first.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 4200
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: { zone_group_1: { x: 44, y: 18 } } }
  const next = migrateG36OptimalStart(saved, reference)
  const learned = next.graph.blocks!.find(block => block.name === 'zone_group_1')!
  assert.equal(learned.blocks!.filter(block => /^optimal_warmup_\d_learner$/.test(block.name)).length, 5)
  assert.equal(learned.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value, 4200)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_group_1, { x: 44, y: 18 })
  for (const group of next.graph.blocks!.filter(block => /^zone_group_[12]$/.test(block.name))) {
    const expected = reference.blocks!.find(block => block.name === group.name)!
    const present = new Set(group.connections!.map(identity))
    for (const edge of expected.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from.split('.')[0].startsWith('optimal_') || to.split('.')[0].startsWith('optimal_'))
        assert.ok(present.has(identity(edge)), `${group.name}: ${from}→${to}`)
    }
    assert.equal(group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'warmup_time_capped.u1').length, 1)
  }
  assert.equal(migrateG36OptimalStart(next, reference), next)
})

test('saved learners continue measuring through occupied operation without losing sample tuning', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => !/^optimal_(warmup|cooldown)_active$/.test(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !/^optimal_(warmup|cooldown)_active$/.test(from.split('.')[0]) &&
        !/^optimal_(warmup|cooldown)_active$/.test(to.split('.')[0])
    })
    for (const mode of ['warmup', 'cooldown']) for (let zone = 1; zone <= 5; zone++)
      group.connections.push(wire(`${mode}_selected.y`, `optimal_${mode}_${zone}_learner.active`))
  }
  const first = old.blocks!.find(block => block.name === 'zone_group_1')!
  first.blocks!.find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters![0].value = 5
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: {} }
  const next = migrateG36OptimalStart(saved, reference)
  const group = next.graph.blocks!.find(block => block.name === 'zone_group_1')!
  assert.equal(group.blocks!.find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters![0].value, 5)
  const incoming = group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_1_learner.active')
  assert.deepEqual(incoming.map(edge => edgeEndpoints(edge)[0]), ['optimal_warmup_active.y'])
  assert.equal(group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_active.clr').length, 1)
  assert.equal(group.blocks!.find(block => block.name === 'optimal_warmup_active')!.block_type.split('.').pop(), 'Latch')
  assert.equal(migrateG36OptimalStart(next, reference), next)
})

test('saved occupancy OR gates become episode latches while preserving learner tuning', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => !/^optimal_(warmup|cooldown)_observation_(window|finished)$/.test(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !/^optimal_(warmup|cooldown)_observation_(window|finished)$/.test(from.split('.')[0]) &&
        !/^optimal_(warmup|cooldown)_observation_(window|finished)$/.test(to.split('.')[0]) &&
        !/^optimal_(warmup|cooldown)_active\.(u|clr)$/.test(to)
    })
    for (const mode of ['warmup', 'cooldown']) {
      const active = group.blocks!.find(block => block.name === `optimal_${mode}_active`)!
      active.block_type = 'Or'
      active.inputs = [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }]
      group.connections.push(wire(`${mode}_selected.y`, `${active.name}.u1`), wire('scheduled_occupied', `${active.name}.u2`))
    }
  }
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36OptimalStartMigration(saved))
  const next = migrateG36OptimalStart(saved, reference)
  const group = next.graph.blocks!.find(block => block.name === 'zone_group_1')!
  const active = group.blocks!.find(block => block.name === 'optimal_warmup_active')!
  assert.equal(active.block_type.split('.').pop(), 'Latch')
  assert.deepEqual(group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_active.u')
    .map(edge => edgeEndpoints(edge)[0]), ['mode_is_warmup.y'])
  assert.deepEqual(group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_active.clr')
    .map(edge => edgeEndpoints(edge)[0]), ['optimal_warmup_observation_finished.y'])
  assert.equal(needsG36OptimalStartMigration(next), false)
})

test('saved learners only complete on measured setpoint attainment', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    for (const learner of group.blocks ?? []) {
      if (!/^optimal_(warmup|cooldown)_[1-5]_learner$/.test(learner.name)) continue
      learner.blocks = learner.blocks!.filter(block => !['not_timed_out', 'reached_before_timeout'].includes(block.name))
      learner.blocks.push({ name: 'done', block_type: 'Or', category: 'elementary', inputs: [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }] })
      learner.connections = learner.connections!.filter(edge => {
        const [from, to] = edgeEndpoints(edge)
        return !['not_timed_out', 'reached_before_timeout'].includes(from.split('.')[0]) &&
          !['not_timed_out', 'reached_before_timeout'].includes(to.split('.')[0])
      })
      learner.connections.push(wire('reached.y', 'done.u1'), wire('recovery_timer.passed', 'done.u2'), wire('done.y', 'active_done.u2'))
      learner.blocks.find(block => block.name === 'recovery_timer')!.parameters![0].value = 10800
    }
  }
  old.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'optimal_warmup_1_learner')!.blocks!
    .find(block => block.name === 'rate_mean')!.parameters![0].value = 5
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36OptimalStartMigration(saved))
  const next = migrateG36OptimalStart(saved, reference)
  const learner = next.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!
    .find(block => block.name === 'optimal_warmup_1_learner')!
  assert.equal(learner.blocks!.find(block => block.name === 'rate_mean')!.parameters![0].value, 5)
  assert.equal(learner.blocks!.find(block => block.name === 'recovery_timer')!.parameters![0].value, 50400)
  assert.ok(!learner.blocks!.some(block => block.name === 'done'))
  assert.deepEqual(learner.connections!.filter(edge => edgeEndpoints(edge)[1] === 'active_done.u2')
    .map(edge => edgeEndpoints(edge)[0]), ['reached_before_timeout.y'])
  assert.equal(needsG36OptimalStartMigration(next), false)
  assert.equal(migrateG36OptimalStart(next, reference), next)
})

test('saved occupied-window observation upgrades to the OBC timed hold', () => {
  const old = structuredClone(reference)
  for (const group of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    group.blocks = group.blocks!.filter(block => !/^optimal_(warmup|cooldown)_horizon_hold$/.test(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !/^optimal_(warmup|cooldown)_horizon_hold(\.|$)/.test(from) &&
        !/^optimal_(warmup|cooldown)_horizon_hold(\.|$)/.test(to)
    })
    for (const mode of ['warmup', 'cooldown']) {
      const window = `optimal_${mode}_observation_window`
      group.blocks.push({ name: window, block_type: 'Or', category: 'elementary',
        inputs: [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }],
        outputs: [{ name: 'y', type: 'Boolean' }], notes: 'Saved operator observation note.' })
      group.connections.push(wire(`optimal_${mode}_horizon.y`, `${window}.u1`),
        wire('scheduled_occupied', `${window}.u2`),
        wire(`${window}.y`, `optimal_${mode}_observation_finished.u`))
    }
  }
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36OptimalStartMigration(saved))
  const next = migrateG36OptimalStart(saved, reference)
  for (const group of next.graph.blocks!.filter(block => /^zone_group_[12]$/.test(block.name))) {
    assert.ok(!group.blocks!.some(block => block.name.endsWith('_observation_window')))
    for (const mode of ['warmup', 'cooldown']) {
      assert.match(group.blocks!.find(block => block.name === `optimal_${mode}_horizon_hold`)!.notes ?? '', /Saved operator observation note/)
      assert.deepEqual(group.connections!.filter(edge => edgeEndpoints(edge)[1] === `optimal_${mode}_horizon_hold.u`)
        .map(edge => edgeEndpoints(edge)[0]), [`optimal_${mode}_horizon.y`])
      assert.deepEqual(group.connections!.filter(edge => edgeEndpoints(edge)[1] === `optimal_${mode}_observation_finished.u`)
        .map(edge => edgeEndpoints(edge)[0]), [`optimal_${mode}_horizon_hold.y`])
    }
  }
  assert.equal(migrateG36OptimalStart(next, reference), next)
})
