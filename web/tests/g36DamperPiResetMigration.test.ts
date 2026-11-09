import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DamperPiReset } from '../src/lib/g36DamperPiResetMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

function project(): Project {
  return { version: 1, name: reference.name, graph: structuredClone(reference), scenario: {} as Project['scenario'], layout: {} }
}

test('upgrades all VAV damper PI blocks while preserving tuning and notes', () => {
  const saved = project()
  for (const zone of saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    const pi = zone.blocks?.find(block => block.name === 'damper_flow_pi')!
    pi.block_type = 'LimPID'
    pi.inputs = pi.inputs?.filter(port => port.name !== 'trigger')
    pi.parameters = pi.parameters?.filter(parameter => !['r', 'Ni', 'y_reset', 'xi_start'].includes(parameter.name))
    zone.connections = zone.connections?.filter(edge => edgeEndpoints(edge)[1] !== 'damper_flow_pi.trigger')
  }
  const original = saved.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'damper_flow_pi')!
  original.parameters!.find(parameter => parameter.name === 'k')!.value = 0.8
  original.notes = 'My tuned loop'
  const migrated = migrateG36DamperPiReset(saved, reference)
  assert.notEqual(migrated, saved)
  assert.equal(original.block_type, 'LimPID')
  for (const zone of migrated.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    const pi = zone.blocks?.find(block => block.name === 'damper_flow_pi')!
    assert.equal(pi.block_type, 'Buildings.Controls.OBC.CDL.Reals.PIDWithReset')
    assert.ok(pi.inputs?.some(port => port.name === 'trigger' && port.type === 'Boolean'))
    assert.ok(zone.connections?.some(edge => edgeEndpoints(edge).join(' -> ') === 'supply_fan_proven -> damper_flow_pi.trigger'))
    assert.ok(pi.parameters?.some(parameter => parameter.name === 'y_reset' && parameter.value === 0.01))
  }
  const tuned = migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'damper_flow_pi')!
  assert.equal(tuned.parameters?.find(parameter => parameter.name === 'k')?.value, 0.8)
  assert.equal(tuned.notes, 'My tuned loop')
  assert.equal(migrateG36DamperPiReset(migrated, reference), migrated)
})
