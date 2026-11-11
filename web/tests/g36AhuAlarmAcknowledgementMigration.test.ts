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
  assert.equal(migrated.graph.outputs?.filter(port => /^ahu_[12]_.*_alarm_acknowledged$/.test(port.name)).length, 20)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')?.notes,
    tuned.notes)
  assert.deepEqual(migrated.layout.ahu_1, {x: 923, y: 158})
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36AhuAlarmAcknowledgement(migrated, reference), migrated)
})

test('existing AHU acknowledgment composites gain both coil-valve alarm channels', () => {
  const graph = structuredClone(reference)
  const coilAlarm = /(heating|cooling)_coil_valve_tracking_alarm/
  for (const group of graph.blocks?.filter(block => block.name.endsWith('_equipment_alarm_acknowledgement')) ?? []) {
    group.inputs = group.inputs?.filter(port => !coilAlarm.test(port.name))
    group.outputs = group.outputs?.filter(port => !coilAlarm.test(port.name))
    group.blocks = group.blocks?.filter(block => !coilAlarm.test(block.name))
    group.connections = group.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint => coilAlarm.test(endpoint)))
  }
  graph.outputs = graph.outputs?.filter(port => !coilAlarm.test(port.name) || !port.name.endsWith('_acknowledged'))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    coilAlarm.test(endpoint) && (endpoint.includes('_equipment_alarm_acknowledgement') || endpoint.endsWith('_acknowledged'))))
  const group = graph.blocks?.find(block => block.name === 'ahu_1_equipment_alarm_acknowledgement')!
  group.blocks!.find(block => block.name === 'supply_fan_alarm_ack_latch')!.notes = 'Preserve existing latch'
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {}} as Project
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AhuAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.outputs?.filter(port => /^ahu_[12]_(heating|cooling)_coil_valve_tracking_alarm_acknowledged$/.test(port.name)).length, 4)
  assert.equal(migrated.graph.blocks?.find(block => block.name === group.name)?.blocks?.find(block =>
    block.name === 'supply_fan_alarm_ack_latch')?.notes, 'Preserve existing latch')
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(migrated), false)
})

test('saved AHU graph gains group alarm channels while preserving tuned acknowledgments', () => {
  const graph = structuredClone(reference)
  const group = graph.blocks?.find(block => block.name === 'ahu_1_equipment_alarm_acknowledgement')!
  const tunedLatch = group.blocks?.find(block => block.name === 'supply_fan_alarm_ack_latch')
  assert.ok(tunedLatch)
  tunedLatch.notes = 'Operator tuning note'
  group.inputs = group.inputs?.filter(port => !port.name.includes('group_window_open_alarm'))
  group.outputs = group.outputs?.filter(port => !port.name.includes('group_window_open_alarm'))
  group.blocks = group.blocks?.filter(block => !block.name.startsWith('group_window_open_alarm_'))
  group.connections = group.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint => endpoint.includes('group_window_open_alarm')))
  graph.outputs = graph.outputs?.filter(port => port.name !== 'ahu_1_group_window_open_alarm_acknowledged')
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    endpoint.includes('ahu_1_group_window_open_alarm_acknowledged') ||
    endpoint === 'ahu_1_equipment_alarm_acknowledgement.ahu_1_group_window_open_alarm_level'))
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {}} as Project
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36AhuAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.find(block => block.name === group.name)?.blocks?.find(block =>
    block.name === 'supply_fan_alarm_ack_latch')?.notes, 'Operator tuning note')
  assert.equal(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') ===
    'zone_group_1.window_open_alarm_level→ahu_1_equipment_alarm_acknowledgement.ahu_1_group_window_open_alarm_level'), true)
  assert.equal(needsG36AhuAlarmAcknowledgementMigration(migrated), false)
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
