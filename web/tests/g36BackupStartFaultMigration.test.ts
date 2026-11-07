import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36BackupStartFault } from '../src/lib/g36BackupStartFaultMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains boiler and chiller start faults without losing tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const added = /^(boiler|chiller)_(start_|not_running$|fault_clear$|can_dispatch$)/
  plant.blocks = plant.blocks!.filter(block => !added.test(block.name))
  plant.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value = 0.3
  old.inputs = old.inputs!.filter(port => !['boiler_fault_reset', 'chiller_fault_reset'].includes(port.name))
  old.outputs = old.outputs!.filter(port => !['plant_boiler_start_fault', 'plant_chiller_start_fault'].includes(port.name))
  plant.inputs = plant.inputs!.filter(port => !['boiler_fault_reset', 'chiller_fault_reset'].includes(port.name))
  plant.outputs = plant.outputs!.filter(port => !['boiler_start_fault', 'chiller_start_fault'].includes(port.name))
  old.connections = old.connections!.filter(edge => !['plant.boiler_fault_reset', 'plant.chiller_fault_reset',
    'plant_boiler_start_fault', 'plant_chiller_start_fault'].includes(edgeEndpoints(edge)[1]))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !added.test(from.split('.')[0]) && !added.test(to.split('.')[0]) &&
      !['boiler_start_fault', 'chiller_start_fault'].includes(to)
  })
  for (const name of ['boiler', 'chiller']) plant.connections.push({from:`${name}_available`,to:`${name}_availability_gate.u3`})
  const saved: Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{plant:{x:42,y:24}}}
  const next = migrateG36BackupStartFault(saved, reference)
  const migrated = next.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(migrated.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value,0.3)
  for (const name of ['boiler','chiller']) {
    assert.ok(migrated.blocks!.some(block => block.name === `${name}_start_fault_latch`))
    assert.equal(migrated.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}_availability_gate.u3`).length,1)
  }
  assert.deepEqual(next.layout.plant,{x:42,y:24})
  assert.equal(migrateG36BackupStartFault(next,reference),next)
})
