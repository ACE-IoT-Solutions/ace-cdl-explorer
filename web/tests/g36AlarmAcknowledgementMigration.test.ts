import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AlarmAcknowledgement, needsG36AlarmAcknowledgementMigration } from '../src/lib/g36AlarmAcknowledgementMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved G36 controls gain editable acknowledgement state without losing tuning or layout', () => {
  const graph = structuredClone(reference)
  graph.blocks = graph.blocks?.filter(block => !block.name.endsWith('_alarm_acknowledgement'))
  graph.inputs = graph.inputs?.filter(port => !port.name.endsWith('_low_airflow_acknowledge'))
  graph.outputs = graph.outputs?.filter(port => !port.name.endsWith('_low_airflow_acknowledged'))
  graph.connections = graph.connections?.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint.includes('_alarm_acknowledgement') ||
      endpoint.endsWith('_low_airflow_acknowledge') || endpoint.endsWith('_low_airflow_acknowledged')))
  const tuned = graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'low_flow_delay')
  if (tuned) tuned.notes = 'Keep my alarm timing note'
  const saved = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: { zone_01: { x: 127, y: 349 } } } as Project

  assert.equal(needsG36AlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.filter(block => block.block_type === 'AlarmAcknowledgement').length, 2)
  assert.equal(migrated.graph.inputs?.filter(port => port.name.endsWith('_low_airflow_acknowledge')).length, 10)
  assert.equal(migrated.graph.outputs?.filter(port => port.name.endsWith('_low_airflow_acknowledged')).length, 10)
  assert.equal(migrated.graph.connections?.filter(edge => edge.to?.includes('_alarm_acknowledgement.')).length, 20)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'low_flow_delay')?.notes, 'Keep my alarm timing note')
  assert.deepEqual(migrated.layout.zone_01, { x: 127, y: 349 })
  assert.equal(needsG36AlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36AlarmAcknowledgement(migrated, reference), migrated)
})

test('a partially saved acknowledgment path repairs its missing boundary wire', () => {
  const graph = structuredClone(reference)
  graph.name = 'My tuned G36 sequence'
  graph.connections = graph.connections?.filter(edge =>
    edgeEndpoints(edge).join('→') !== 'zone_01_low_airflow_acknowledge→ahu_1_alarm_acknowledgement.acknowledge_1')
  const saved = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {} } as Project
  assert.equal(needsG36AlarmAcknowledgementMigration(saved), true)
  const repaired = migrateG36AlarmAcknowledgement(saved, reference)
  assert.equal(repaired.graph.connections?.filter(edge => edgeEndpoints(edge).join('→') ===
    'zone_01_low_airflow_acknowledge→ahu_1_alarm_acknowledgement.acknowledge_1').length, 1)
  assert.equal(needsG36AlarmAcknowledgementMigration(repaired), false)
})
