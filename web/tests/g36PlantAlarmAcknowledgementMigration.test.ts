import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PlantAlarmAcknowledgement, needsG36PlantAlarmAcknowledgementMigration } from '../src/lib/g36PlantAlarmAcknowledgementMigration'
import { edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))

test('saved G36 plant gains seven editable fault acknowledgments without losing tuning', () => {
  const graph = structuredClone(reference)
  graph.blocks = graph.blocks?.filter(block => block.name !== 'plant_alarm_acknowledgement')
  graph.inputs = graph.inputs?.filter(port => port.name !== 'plant_alarm_acknowledge')
  graph.outputs = graph.outputs?.filter(port => !port.name.endsWith('_fault_acknowledged'))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    endpoint.includes('plant_alarm_acknowledgement') || endpoint === 'plant_alarm_acknowledge' ||
    endpoint.endsWith('_fault_acknowledged')))
  const plant = graph.blocks?.find(block => block.name === 'plant')!
  plant.notes = 'Keep my plant notes'
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: {plant: {x: 73, y: 119}}} as Project
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36PlantAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.blocks?.filter(block => block.name === 'plant_alarm_acknowledgement').length, 1)
  assert.equal(migrated.graph.outputs?.filter(port => port.name.endsWith('_fault_acknowledged')).length, 7)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'plant')?.notes, 'Keep my plant notes')
  assert.deepEqual(migrated.layout.plant, {x: 73, y: 119})
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(migrated), false)
  assert.equal(migrateG36PlantAlarmAcknowledgement(migrated, reference), migrated)
})

test('a partial plant acknowledgment channel is repaired without replacing its composite', () => {
  const graph = structuredClone(reference)
  graph.connections = graph.connections?.filter(edge => edgeEndpoints(edge).join('→') !==
    'plant.hot_water_flow_fault→plant_alarm_acknowledgement.hot_water_flow_fault')
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {}} as Project
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36PlantAlarmAcknowledgement(saved, reference)
  assert.equal(migrated.graph.connections?.filter(edge => edgeEndpoints(edge).join('→') ===
    'plant.hot_water_flow_fault→plant_alarm_acknowledgement.hot_water_flow_fault').length, 1)
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(migrated), false)
})

test('three-channel saved composite gains pump and flow faults while keeping edited blocks', () => {
  const graph = structuredClone(reference)
  const group = graph.blocks?.find(block => block.name === 'plant_alarm_acknowledgement')!
  const addedFaults = ['hot_water_pump_start_fault', 'chilled_water_pump_start_fault',
    'hot_water_flow_fault', 'chilled_water_flow_fault']
  group.inputs = group.inputs?.filter(port => !addedFaults.includes(port.name))
  group.outputs = group.outputs?.filter(port => !addedFaults.some(fault => port.name === `${fault}_acknowledged`))
  group.blocks = group.blocks?.filter(block => !addedFaults.some(fault => block.name.startsWith(`${fault}_`)))
  group.connections = group.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    addedFaults.some(fault => endpoint.startsWith(`${fault}_`))))
  group.notes = 'Keep my acknowledgement notes'
  group.blocks!.find(block => block.name === 'ack_edge')!.notes = 'Keep my edge notes'
  graph.outputs = graph.outputs?.filter(port => !addedFaults.some(fault => port.name === `plant_${fault}_acknowledged`))
  graph.connections = graph.connections?.filter(edge => !edgeEndpoints(edge).some(endpoint =>
    addedFaults.some(fault => endpoint.startsWith('plant_alarm_acknowledgement.') && endpoint.includes(fault) ||
      endpoint === `plant_${fault}_acknowledged`)))
  const saved = {version: 1, name: graph.name, graph, scenario: {} as Project['scenario'],
    layout: {plant_alarm_acknowledgement: {x: 17, y: 41}}} as Project
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(saved), true)
  const migrated = migrateG36PlantAlarmAcknowledgement(saved, reference)
  const result = migrated.graph.blocks?.find(block => block.name === 'plant_alarm_acknowledgement')!
  assert.equal(result.notes, 'Keep my acknowledgement notes')
  assert.equal(result.blocks?.find(block => block.name === 'ack_edge')?.notes, 'Keep my edge notes')
  assert.deepEqual(migrated.layout.plant_alarm_acknowledgement, {x: 17, y: 41})
  assert.equal(migrated.graph.outputs?.filter(port => port.name.endsWith('_fault_acknowledged')).length, 7)
  assert.equal(needsG36PlantAlarmAcknowledgementMigration(migrated), false)
})
