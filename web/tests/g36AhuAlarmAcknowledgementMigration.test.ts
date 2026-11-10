import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AhuAlarmAcknowledgement, needsG36AhuAlarmAcknowledgementMigration } from '../src/lib/g36AhuAlarmAcknowledgementMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved AHU alarm graphs gain CDL acknowledgment state without losing controls', () => {
  const graph = structuredClone(reference)
  graph.blocks = graph.blocks?.filter(block => !block.name.endsWith('_equipment_alarm_acknowledgement'))
  graph.inputs = graph.inputs?.filter(port => !/^ahu_[12]_alarm_acknowledge$/.test(port.name))
  graph.outputs = graph.outputs?.filter(port => !/^ahu_[12]_.*_alarm_acknowledged$/.test(port.name))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    endpoint.includes('_equipment_alarm_acknowledgement') ||
    /^ahu_[12]_alarm_acknowledge$/.test(endpoint) ||
    /^ahu_[12]_.*_alarm_acknowledged$/.test(endpoint)))
  const tuned = graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')!
  tuned.notes = 'Keep my fan tuning note'
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: {ahu_1: {x: 923, y: 158}}} as Project
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AhuAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.filter(block => block.name.endsWith('_equipment_alarm_acknowledgement')).length, 2)
  assert.equal(migrated.graph.inputs?.filter(port => /^ahu_[12]_alarm_acknowledge$/.test(port.name)).length, 2)
  assert.equal(migrated.graph.outputs?.filter(port => /^ahu_[12]_.*_alarm_acknowledged$/.test(port.name)).length, 12)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')?.notes,
    tuned.notes)
  assert.deepEqual(migrated.layout.ahu_1, {x: 923, y: 158})
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36AhuAlarmAcknowledgement(migrated, reference), migrated)
})

test('a missing AHU acknowledgment wire is repaired without replacing its composite', () => {
  const graph = structuredClone(reference)
  graph.name = 'My AHU sequence'
  graph.connections = graph.connections?.filter(edge => edgeEndpoints(edge).join('→') !==
    'ahu_1_alarm_acknowledge→ahu_1_equipment_alarm_acknowledgement.acknowledge')
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {}} as Project
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AhuAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.connections?.filter(edge => edgeEndpoints(edge).join('→') ===
    'ahu_1_alarm_acknowledge→ahu_1_equipment_alarm_acknowledgement.acknowledge').length, 1)
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(migrated), false)
})
