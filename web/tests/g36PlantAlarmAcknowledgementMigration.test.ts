import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PlantAlarmAcknowledgement, needsG36PlantAlarmAcknowledgementMigration } from '../src/lib/g36PlantAlarmAcknowledgementMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved G36 plant gains editable source fault acknowledgment without losing tuning', () => {
  const graph = structuredClone(reference)
  graph.blocks = graph.blocks?.filter(block => block.name !== 'plant_alarm_acknowledgement')
  graph.inputs = graph.inputs?.filter(port => port.name !== 'plant_alarm_acknowledge')
  graph.outputs = graph.outputs?.filter(port => !port.name.endsWith('_start_fault_acknowledged'))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    endpoint.includes('plant_alarm_acknowledgement') || endpoint === 'plant_alarm_acknowledge' ||
    endpoint.endsWith('_start_fault_acknowledged')))
  const plant = graph.blocks?.find(block => block.name === 'plant')!
  plant.notes = 'Keep my plant notes'
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: {plant: {x: 73, y: 119}}} as Project
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36PlantAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.filter(block => block.name === 'plant_alarm_acknowledgement').length, 1)
  assert.equal(migrated.graph.outputs?.filter(port => port.name.endsWith('_start_fault_acknowledged')).length, 3)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'plant')?.notes, 'Keep my plant notes')
  assert.deepEqual(migrated.layout.plant, {x: 73, y: 119})
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36PlantAlarmAcknowledgement(migrated, reference), migrated)
})

test('a partial plant acknowledgment channel is repaired without replacing its composite', () => {
  const graph = structuredClone(reference)
  graph.connections = graph.connections?.filter(edge => edgeEndpoints(edge).join('→') !==
    'plant.boiler_start_fault→plant_alarm_acknowledgement.boiler_start_fault')
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {}} as Project
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36PlantAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.connections?.filter(edge => edgeEndpoints(edge).join('→') ===
    'plant.boiler_start_fault→plant_alarm_acknowledgement.boiler_start_fault').length, 1)
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(migrated), false)
})
