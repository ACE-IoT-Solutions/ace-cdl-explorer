import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterPumpStartFault } from '../src/lib/g36WaterPumpStartFaultMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 graph gains editable water pump faults without replacing source tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const fault = (name: string) => name.startsWith('hot_pump_fault_') || name.startsWith('chilled_pump_fault_')
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_water_pump_start_fault'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_water_pump_start_fault'))
  plant.outputs = plant.outputs?.filter(port => !port.name.endsWith('_water_pump_start_fault'))
  plant.blocks = plant.blocks?.filter(block => !fault(block.name))
  plant.connections = plant.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !fault(from.split('.')[0]) && !fault(to.split('.')[0]) && !to.endsWith('_water_pump_start_fault')
  })
  plant.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value = 0.25
  const saved: Project = { version:1, name:old.name, graph:old, scenario:{...defaultScenario}, layout:{plant:{x:21,y:42}} }
  const migrated = migrateG36WaterPumpStartFault(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value, 0.25)
  assert.equal(upgraded.blocks!.some(block => block.name === 'hot_pump_fault_delay'), true)
  assert.equal(upgraded.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_pump_fault_delay.y→hot_water_pump_start_fault'), true)
  assert.equal(migrated.graph.outputs!.some(port => port.name === 'plant_chilled_water_pump_start_fault'), true)
  assert.deepEqual(migrated.layout.plant, {x:21,y:42})
  assert.equal(migrateG36WaterPumpStartFault(migrated, reference), migrated)
})
