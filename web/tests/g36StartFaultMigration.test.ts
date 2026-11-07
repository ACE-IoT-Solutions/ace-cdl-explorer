import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36StartFault } from '../src/lib/g36StartFaultMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains CDL start fault without losing dispatch tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  plant.blocks = plant.blocks!.filter(block => !block.name.startsWith('heat_pump_start_') &&
    !['heat_pump_not_running', 'heat_pump_fault_clear', 'heat_pump_can_dispatch'].includes(block.name))
  plant.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value = 0.4
  old.inputs = old.inputs!.filter(port => port.name !== 'heat_pump_fault_reset')
  old.outputs = old.outputs!.filter(port => port.name !== 'plant_heat_pump_start_fault')
  plant.inputs = plant.inputs!.filter(port => port.name !== 'heat_pump_fault_reset')
  plant.outputs = plant.outputs!.filter(port => port.name !== 'heat_pump_start_fault')
  old.connections = old.connections!.filter(edge => !['plant.heat_pump_fault_reset', 'plant_heat_pump_start_fault'].includes(edgeEndpoints(edge)[1]))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !from.startsWith('heat_pump_start_') && !to.startsWith('heat_pump_start_') &&
      !['heat_pump_not_running', 'heat_pump_fault_clear', 'heat_pump_can_dispatch'].some(name => from.startsWith(name) || to.startsWith(name))
  })
  plant.connections.push({from:'heat_pump_available',to:'heat_pump_availability_gate.u3'})
  const saved: Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{plant:{x:42,y:24}}}
  const next = migrateG36StartFault(saved, reference)
  const nextPlant = next.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(nextPlant.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.4)
  assert.equal(nextPlant.connections!.filter(edge => edgeEndpoints(edge)[1] === 'heat_pump_availability_gate.u3').length, 1)
  assert.ok(nextPlant.blocks!.some(block => block.name === 'heat_pump_start_fault_latch'))
  assert.deepEqual(next.layout.plant,{x:42,y:24})
  assert.equal(migrateG36StartFault(next,reference),next)
})
