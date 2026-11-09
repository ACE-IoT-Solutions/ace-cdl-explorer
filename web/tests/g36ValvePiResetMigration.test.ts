import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ValvePiReset, needsG36ValvePiResetMigration } from '../src/lib/g36ValvePiResetMigration'
import type { Block, Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved G36 reheat loops gain enable-triggered reset while retaining tuning and notes', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    const pi = zone.blocks?.find(block => block.name === 'discharge_valve_pi')!
    pi.block_type = 'LimPID'
    pi.inputs = pi.inputs?.filter(port => port.name !== 'trigger')
    pi.parameters = pi.parameters?.filter(parameter => !['r', 'Ni', 'y_reset', 'xi_start'].includes(parameter.name))
    zone.connections = zone.connections?.filter(edge => edge.to !== 'discharge_valve_pi.trigger')
  }
  const first = old.blocks?.find(block => block.name === 'zone_01')!
  const tuned = first.blocks?.find(block => block.name === 'discharge_valve_pi')!
  tuned.parameters!.find(parameter => parameter.name === 'k')!.value = 0.08
  tuned.parameters!.find(parameter => parameter.name === 'Ti')!.value = 600
  tuned.notes = 'Custom reheat tuning'
  const saved = {version: 1, name: old.name, graph: old, scenario: {} as Project['scenario'], layout: {zone_01: {x: 120, y: 340}}} as Project
  assert.equal(needsG36ValvePiResetMigration(saved), true)
  const migrated = migrateG36ValvePiReset(saved, reference)
  for (const zone of migrated.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    const pi = zone.blocks?.find(block => block.name === 'discharge_valve_pi')!
    assert.equal(pi.block_type, 'Buildings.Controls.OBC.CDL.Reals.PIDWithReset')
    assert.ok(pi.inputs?.some(port => port.name === 'trigger' && port.type === 'Boolean'))
    assert.ok(pi.parameters?.some(parameter => parameter.name === 'y_reset' && parameter.value === 0))
    assert.ok(zone.connections?.some(edge => edge.from === 'discharge_valve_enable.y' && edge.to === 'discharge_valve_pi.trigger'))
  }
  const updated = migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'discharge_valve_pi')!
  assert.equal(updated.parameters?.find(parameter => parameter.name === 'k')?.value, 0.08)
  assert.equal(updated.parameters?.find(parameter => parameter.name === 'Ti')?.value, 600)
  assert.equal(updated.notes, 'Custom reheat tuning')
  assert.deepEqual(migrated.layout.zone_01, {x: 120, y: 340})
  assert.equal(needsG36ValvePiResetMigration(migrated), false)
  assert.equal(migrateG36ValvePiReset(migrated, reference), migrated)
})
