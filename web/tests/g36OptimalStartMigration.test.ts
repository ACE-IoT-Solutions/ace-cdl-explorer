import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OptimalStart } from '../src/lib/g36OptimalStartMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
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
  assert.equal(group.connections!.filter(edge => edgeEndpoints(edge)[1] === 'optimal_warmup_active.u2').length, 1)
  assert.equal(migrateG36OptimalStart(next, reference), next)
})
