import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DamperModeGate, needsG36DamperModeGateMigration } from '../src/lib/g36DamperModeGateMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('migrates old zero-flow damper gate without changing tuned PI blocks', () => {
  const saved: Project = { version: 1, name: reference.name, graph: structuredClone(reference), scenario: {} as Project['scenario'], layout: {} }
  for (const zone of saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.blocks = zone.blocks?.filter(block => !['damper_is_unoccupied', 'damper_operation_enabled'].includes(block.name))
    zone.connections = zone.connections?.filter(edge => edgeEndpoints(edge)[1] === 'damper_feedback_output.u3' || !['damper_is_unoccupied', 'damper_operation_enabled'].some(name => edgeEndpoints(edge).some(end => end.startsWith(`${name}.`))))
    const gate = zone.connections?.find(edge => edgeEndpoints(edge)[1] === 'damper_feedback_output.u3')
    assert.ok(gate)
    gate.from = 'damper_flow_requested.y'
  }
  const pi = saved.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'damper_flow_pi')
  assert.ok(pi)
  const gain = pi.parameters?.find(parameter => parameter.name === 'k')
  assert.ok(gain)
  gain.value = 0.8
  assert.equal(needsG36DamperModeGateMigration(saved), true)
  const migrated = migrateG36DamperModeGate(saved, reference)
  assert.notEqual(migrated, saved)
  for (const zone of migrated.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    assert.deepEqual(zone.connections?.filter(edge => edgeEndpoints(edge)[1] === 'damper_feedback_output.u3').map(edge => edgeEndpoints(edge)[0]), ['damper_operation_enabled.y'])
    for (const name of ['damper_is_unoccupied', 'damper_operation_enabled']) assert.ok(zone.blocks?.some(block => block.name === name))
  }
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'damper_flow_pi')?.parameters?.find(parameter => parameter.name === 'k')?.value, 0.8)
  assert.equal(needsG36DamperModeGateMigration(migrated), false)
  assert.equal(migrateG36DamperModeGate(migrated, reference), migrated)
})
