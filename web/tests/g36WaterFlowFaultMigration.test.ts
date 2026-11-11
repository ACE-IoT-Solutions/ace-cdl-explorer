import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterFlowFault } from '../src/lib/g36WaterFlowFaultMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains timed water-flow diagnostics without replacing proof tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_water_flow_fault'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_water_flow_fault'))
  plant.outputs = plant.outputs?.filter(port => !port.name.endsWith('_water_flow_fault'))
  plant.blocks = plant.blocks?.filter(block => !block.name.includes('_flow_fault_'))
  plant.connections = plant.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !from.includes('_flow_fault_') && !to.includes('_flow_fault_') && !to.endsWith('_water_flow_fault')
  })
  plant.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value = 0.25
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 21, y: 42 } } }
  const migrated = migrateG36WaterFlowFault(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value, 0.25)
  assert.equal(upgraded.blocks!.some(block => block.name === 'hot_flow_fault_delay'), true)
  assert.equal(upgraded.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_flow_fault_delay.y→hot_water_flow_fault'), true)
  assert.equal(migrated.graph.outputs!.some(port => port.name === 'plant_chilled_water_flow_fault'), true)
  assert.deepEqual(migrated.layout.plant, { x: 21, y: 42 })
  assert.equal(migrateG36WaterFlowFault(migrated, reference), migrated)
})

test('a custom flow-fault input is preserved while missing output wiring is repaired', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  plant.connections = plant.connections!.filter(edge => !['hot_flow_fault_unproven.u2', 'hot_water_flow_fault'].includes(edgeEndpoints(edge)[1]))
  plant.connections.push({ from: 'hot_source_motor_and_flow.y', to: 'hot_flow_fault_unproven.u2' })
  old.connections = old.connections!.filter(edge => edgeEndpoints(edge)[1] !== 'plant_hot_water_flow_fault')
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: {} }
  const migrated = migrateG36WaterFlowFault(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_source_motor_and_flow.y→hot_flow_fault_unproven.u2'), true)
  assert.equal(upgraded.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_flow_fault_delay.y→hot_water_flow_fault'), true)
  assert.equal(migrated.graph.connections!.some(edge => edgeEndpoints(edge).join('→') === 'plant.hot_water_flow_fault→plant_hot_water_flow_fault'), true)
})
