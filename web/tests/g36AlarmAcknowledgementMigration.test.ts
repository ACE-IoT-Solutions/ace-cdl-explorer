import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AlarmAcknowledgement, needsG36AlarmAcknowledgementMigration } from '../src/lib/g36AlarmAcknowledgementMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved G36 controls gain editable acknowledgement state without losing tuning or layout', () => {
  const graph = structuredClone(reference)
  graph.blocks = graph.blocks?.filter(block => !block.name.endsWith('_alarm_acknowledgement'))
  graph.inputs = graph.inputs?.filter(port => !port.name.endsWith('_alarm_acknowledge'))
  graph.outputs = graph.outputs?.filter(port => !port.name.endsWith('_low_airflow_acknowledged'))
  graph.connections = graph.connections?.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint.includes('_alarm_acknowledgement') ||
      endpoint.endsWith('_alarm_acknowledge') || endpoint.endsWith('_low_airflow_acknowledged')))
  const tuned = graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'low_flow_delay')
  if (tuned) tuned.notes = 'Keep my alarm timing note'
  const saved = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: { zone_01: { x: 127, y: 349 } } } as Project

  assert.equal(needsG36AlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.filter(block => block.block_type === 'AlarmAcknowledgement').length, 2)
  assert.equal(migrated.graph.inputs?.filter(port => port.name.endsWith('_alarm_acknowledge')).length, 10)
  assert.equal(migrated.graph.outputs?.filter(port => port.name.endsWith('_low_airflow_acknowledged')).length, 10)
  assert.equal(migrated.graph.outputs?.filter(port => /^zone_\d{2}_.*_alarm_acknowledged$/.test(port.name)).length, 40)
  assert.equal(migrated.graph.connections?.filter(edge => edge.to?.includes('_alarm_acknowledgement.')).length, 60)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'low_flow_delay')?.notes, 'Keep my alarm timing note')
  assert.deepEqual(migrated.layout.zone_01, { x: 127, y: 349 })
  assert.equal(needsG36AlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36AlarmAcknowledgement(migrated, reference), migrated)
})

test('a partially saved acknowledgment path repairs its missing boundary wire', () => {
  const graph = structuredClone(reference)
  graph.name = 'My tuned G36 sequence'
  graph.connections = graph.connections?.filter(edge =>
    edgeEndpoints(edge).join('→') !== 'zone_01_alarm_acknowledge→ahu_1_alarm_acknowledgement.acknowledge_1')
  const saved = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {} } as Project
  assert.equal(needsG36AlarmAcknowledgementMigration(saved), true)
  const repaired = migrateG36AlarmAcknowledgement(saved, reference)
  assert.equal(repaired.graph.connections?.filter(edge => edgeEndpoints(edge).join('→') ===
    'zone_01_alarm_acknowledge→ahu_1_alarm_acknowledgement.acknowledge_1').length, 1)
  assert.equal(needsG36AlarmAcknowledgementMigration(repaired), false)
})

test('older low-airflow acknowledgment composites gain other alarm channels without losing edits', () => {
  const graph = structuredClone(reference)
  graph.inputs = graph.inputs?.map(port => port.name.endsWith('_alarm_acknowledge')
    ? {...port, name: port.name.replace(/_alarm_acknowledge$/, '_low_airflow_acknowledge')} : port)
  graph.connections = graph.connections?.map(edge => edge.from?.endsWith('_alarm_acknowledge')
    ? {...edge, from: edge.from.replace(/_alarm_acknowledge$/, '_low_airflow_acknowledge')} : edge)
  const extra = ['airflow_sensor_alarm', 'low_discharge_temperature_alarm', 'leaking_damper_alarm', 'leaking_reheat_valve_alarm']
  for (const group of graph.blocks?.filter(block => block.block_type === 'AlarmAcknowledgement') ?? []) {
    group.inputs = group.inputs?.filter(port => !extra.some(kind => port.name.startsWith(kind)))
    group.outputs = group.outputs?.filter(port => !extra.some(kind => port.name.startsWith(kind)))
    group.blocks = group.blocks?.filter(block => !extra.some(kind => block.name.includes(kind)))
    group.connections = group.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint => extra.some(kind => endpoint.includes(kind))))
  }
  graph.outputs = graph.outputs?.filter(port => !extra.some(kind => port.name.includes(kind) && port.name.endsWith('_acknowledged')))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    extra.some(kind => endpoint.includes(kind) && (endpoint.includes('_acknowledgement.') || endpoint.endsWith('_acknowledged')))))
  const first = graph.blocks?.find(block => block.name === 'ahu_1_alarm_acknowledgement')!
  first.blocks!.find(block => block.name === 'zone_01_ack_latch')!.notes = 'Keep my operator workflow note'
  const saved = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {} } as Project
  assert.equal(needsG36AlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.find(block => block.name === first.name)?.blocks?.find(block => block.name === 'zone_01_ack_latch')?.notes,
    'Keep my operator workflow note')
  assert.equal(migrated.graph.blocks?.find(block => block.name === first.name)?.blocks?.filter(block => block.name === 'zone_01_airflow_sensor_alarm_ack_latch').length, 1)
  assert.equal(migrated.graph.inputs?.some(port => port.name === 'zone_01_low_airflow_acknowledge'), false)
  assert.equal(migrated.graph.inputs?.some(port => port.name === 'zone_01_alarm_acknowledge'), true)
  assert.equal(needsG36AlarmAcknowledgementMigration(migrated), false)
})
