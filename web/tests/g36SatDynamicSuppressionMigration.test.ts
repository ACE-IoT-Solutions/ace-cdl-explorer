import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SatDynamicSuppression, needsG36SatDynamicSuppressionMigration } from '../src/lib/g36SatDynamicSuppressionMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const fixed = JSON.parse(readFileSync(new URL('./fixtures/g36_fixed_sat_zone.json', import.meta.url), 'utf8')) as Pick<Block, 'blocks' | 'connections'>

test('previously saved fixed SAT suppression gains the captured-error duration without losing tuned request thresholds', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(child => child.name === 'zone_01')!
  zone.blocks = [...zone.blocks!.filter(child => !child.name.startsWith('sat_')), ...fixed.blocks!]
  zone.connections = [...zone.connections!.filter(edge => {
    const target = edgeEndpoints(edge)[1]
    return !target.startsWith('sat_') && target !== 'cooling_sat_reset_request'
  }), ...fixed.connections!]
  zone.blocks.find(child => child.name === 'sat_two_error_threshold')!.parameters![0].value = 2.4
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36SatDynamicSuppressionMigration(saved))
  const next = migrateG36SatDynamicSuppression(saved, reference)
  const updated = next.graph.blocks!.find(child => child.name === 'zone_01')!
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.equal(updated.blocks!.find(child => child.name === 'sat_two_error_threshold')!.parameters![0].value, 2.4)
  assert.equal(updated.blocks!.find(child => child.name === 'sat_suppression_rate')!.parameters![0].value, 540)
  assert.equal(updated.blocks!.find(child => child.name === 'sat_suppression_maximum')!.parameters![0].value, 1800)
  assert.ok(!updated.blocks!.some(child => child.name === 'sat_suppression_elapsed'))
  for (const target of ['sat_two_eligible.u2', 'sat_three_eligible.u2']) {
    const edges = updated.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
    assert.deepEqual(edges.map(edge => edgeEndpoints(edge)[0]), ['sat_suppression_complete.y'])
  }
  assert.equal(needsG36SatDynamicSuppressionMigration(next), false)
  assert.equal(migrateG36SatDynamicSuppression(next, reference), next)
})

test('saved variable-duration suppression gains OBC startup qualification without replacing tuning', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(child => child.name === 'zone_01')!
  const added = new Set(['sat_startup_true', 'sat_startup_delay', 'sat_change_after_startup',
    'sat_change_latch', 'sat_latch_inactive', 'sat_suppression_complete'])
  zone.blocks = zone.blocks!.filter(child => !added.has(child.name))
  zone.connections = zone.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
  }).filter(edge => edgeEndpoints(edge)[1] !== 'sat_change_edge.u')
  zone.connections.push({ from: 'sat_setpoint_changed.y', to: 'sat_change_edge.u' })
  for (const target of ['sat_two_eligible.u2', 'sat_three_eligible.u2'])
    zone.connections.push({ from: 'sat_suppression_passed.y', to: target })
  zone.blocks.find(child => child.name === 'sat_suppression_rate')!.parameters![0].value = 600
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 23, y: 34 } } }
  assert.ok(needsG36SatDynamicSuppressionMigration(saved))
  const next = migrateG36SatDynamicSuppression(saved, reference)
  const updated = next.graph.blocks!.find(child => child.name === 'zone_01')!
  assert.deepEqual(next.layout.zone_01, { x: 23, y: 34 })
  assert.equal(updated.blocks!.find(child => child.name === 'sat_suppression_rate')!.parameters![0].value, 600)
  assert.ok(updated.blocks!.some(child => child.name === 'sat_startup_delay'))
  assert.ok(updated.blocks!.some(child => child.name === 'sat_change_latch'))
  assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sat_change_edge.u')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_change_latch.y'])
  assert.equal(needsG36SatDynamicSuppressionMigration(next), false)
})

test('saved startup-qualified suppression gains a latched first-change interval', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(child => child.name === 'zone_01')!
  const obsolete = new Set(['sat_change_latch', 'sat_clear_to_real', 'sat_clear_previous',
    'sat_clear_boolean', 'sat_latch_inactive'])
  zone.blocks = zone.blocks!.filter(child => !obsolete.has(child.name))
  zone.blocks.push({ name: 'sat_before_startup', block_type: 'Buildings.Controls.OBC.CDL.Logical.Not',
    inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }] })
  zone.blocks.push({ name: 'sat_suppression_timer_gate', block_type: 'Buildings.Controls.OBC.CDL.Logical.Not',
    inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }] })
  zone.connections = zone.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !obsolete.has(from.split('.')[0]) && !obsolete.has(to.split('.')[0])
  })
  zone.connections.push(
    { from: 'sat_change_after_startup.y', to: 'sat_change_edge.u' },
    { from: 'sat_change_edge.y', to: 'sat_suppression_timer_gate.u' },
    { from: 'sat_suppression_timer_gate.y', to: 'sat_suppression_timer.u' },
    { from: 'sat_startup_delay.y', to: 'sat_before_startup.u' },
    { from: 'sat_before_startup.y', to: 'sat_suppression_complete.u1' },
  )
  zone.blocks.find(child => child.name === 'sat_suppression_rate')!.parameters![0].value = 615
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 38, y: 42 } } }
  assert.ok(needsG36SatDynamicSuppressionMigration(saved))
  const next = migrateG36SatDynamicSuppression(saved, reference)
  const updated = next.graph.blocks!.find(child => child.name === 'zone_01')!
  assert.deepEqual(next.layout.zone_01, { x: 38, y: 42 })
  assert.equal(updated.blocks!.find(child => child.name === 'sat_suppression_rate')!.parameters![0].value, 615)
  assert.ok(!updated.blocks!.some(child =>
    ['sat_before_startup', 'sat_suppression_timer_gate'].includes(child.name)))
  assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sat_change_edge.u')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_change_latch.y'])
  assert.equal(needsG36SatDynamicSuppressionMigration(next), false)
})
