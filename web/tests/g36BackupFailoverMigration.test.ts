import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36BackupFailover, needsG36BackupFailoverMigration } from '../src/lib/g36BackupFailoverMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block
const newNames = new Set(['heat_pump_unavailable','heat_pump_failover',
  'boiler_failover_start','boiler_qualified_start','chiller_failover_start','chiller_qualified_start'])

function prior():Project {
  const graph = structuredClone(reference)
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  plant.blocks = plant.blocks!.filter(block => !newNames.has(block.name))
  plant.connections = plant.connections!.filter(edge => {
    const [from,to] = edgeEndpoints(edge)
    return !newNames.has(from.split('.')[0]) && !newNames.has(to.split('.')[0]) &&
      !to.endsWith('_minimum_run.u')
  })
  for (const source of ['boiler','chiller']) plant.connections.push({from:`${source}_stage_up_delay.y`,to:`${source}_minimum_run.u`})
  plant.blocks.find(block => block.name === 'boiler_stage_up_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value = 420
  return {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{plant:{x:19,y:27}}}
}

test('saved G36 backup stages gain CDL fast failover without replacing tuning', () => {
  const saved = prior()
  assert.equal(needsG36BackupFailoverMigration(saved),true)
  const upgraded = migrateG36BackupFailover(saved,reference)
  const plant = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.deepEqual(upgraded.layout.plant,{x:19,y:27})
  assert.equal(plant.blocks!.find(block => block.name === 'boiler_stage_up_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value,420)
  for (const source of ['boiler','chiller']) {
    assert.equal(plant.connections!.find(edge => edgeEndpoints(edge)[1] === `${source}_minimum_run.u`)!.from,
      `${source}_qualified_start.y`)
    for (const target of [`${source}_failover_start.u1`,`${source}_failover_start.u2`,
      `${source}_qualified_start.u1`,`${source}_qualified_start.u2`])
      assert.equal(plant.connections!.filter(edge => edgeEndpoints(edge)[1] === target).length,1,target)
  }
  assert.equal(needsG36BackupFailoverMigration(upgraded),false)
  assert.equal(migrateG36BackupFailover(upgraded,reference),upgraded)
})

test('custom source command is not given a disconnected failover branch', () => {
  const saved = prior()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_start_command')!.from = 'custom_start.y'
  const upgraded = migrateG36BackupFailover(saved,reference)
  const next = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_minimum_run.u')!.from,
    'boiler_stage_up_delay.y')
  assert.ok(!next.blocks!.some(block => block.name === 'boiler_qualified_start'))
  assert.ok(next.blocks!.some(block => block.name === 'chiller_qualified_start'))
})

test('partial failover wiring is repaired without replacing existing blocks', () => {
  const saved:Project = {version:1,name:reference.name,graph:structuredClone(reference),
    scenario:structuredClone(defaultScenario),layout:{}}
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections = plant.connections!.filter(edge => ![
    'heat_pump_failover.u2','boiler_stage_up_delay.u',
  ].includes(edgeEndpoints(edge)[1]))
  assert.equal(needsG36BackupFailoverMigration(saved),true)
  const repaired = migrateG36BackupFailover(saved,reference)
  const next = repaired.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'heat_pump_failover.u2')!.from,
    'heat_pump_start_fault_latch.y')
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_stage_up_delay.u')!.from,
    'boiler_start_positive.y')
  assert.equal(needsG36BackupFailoverMigration(repaired),false)
})
