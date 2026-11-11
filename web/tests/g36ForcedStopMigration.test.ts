import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ForcedStop, needsG36ForcedStopMigration } from '../src/lib/g36ForcedStopMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url),'utf8')) as Block

function prior():Project {
  const graph = structuredClone(reference)
  graph.name = 'Edited G36 plant'
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  const added = new Set(['heat_pump','boiler','chiller'].flatMap(source => [
    `${source}_stop_edge`,`${source}_forced_stop_lockout`,`${source}_forced_stop_clear`,`${source}_restart_gate`,
  ]))
  plant.blocks = plant.blocks!.filter(block => !added.has(block.name))
  plant.blocks.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'trueHoldDuration')!.value = 420
  plant.connections = plant.connections!.filter(edge => {
    const [from,to] = edgeEndpoints(edge)
    return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) &&
      !to.endsWith('_start_command') && !to.endsWith('_issued_start_real.u')
  })
  for (const source of ['heat_pump','boiler','chiller']) {
    plant.connections.push({from:`${source}_start_gate.y`,to:`${source}_start_command`})
    plant.connections.push({from:`${source}_start_gate.y`,to:`${source}_issued_start_real.u`})
  }
  return {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{plant:{x:24,y:36}}}
}

test('saved G36 plant gains a physical-stop lockout without replacing source tuning', () => {
  const saved = prior()
  assert.equal(needsG36ForcedStopMigration(saved),true)
  const migrated = migrateG36ForcedStop(saved,reference)
  const plant = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(plant.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'trueHoldDuration')!.value,420)
  assert.deepEqual(migrated.layout.plant,{x:24,y:36})
  for (const source of ['heat_pump','boiler','chiller']) {
    const lockout = plant.blocks!.find(block => block.name === `${source}_forced_stop_lockout`)!
    assert.equal(lockout.parameters!.find(parameter => parameter.name === 'trueHoldDuration')!.value,300)
    for (const connection of [
      `${source}_running→${source}_stop_edge.u`,
      `${source}_stop_edge.y→${source}_forced_stop_lockout.u`,
      `${source}_restart_gate.y→${source}_start_command`,
      `${source}_restart_gate.y→${source}_issued_start_real.u`,
    ]) assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === connection),connection)
  }
  assert.equal(needsG36ForcedStopMigration(migrated),false)
  assert.equal(migrateG36ForcedStop(migrated,reference),migrated)
})

test('custom source start driver remains untouched', () => {
  const saved = prior()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_start_command')!.from = 'hot_water_enable'
  const migrated = migrateG36ForcedStop(saved,reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_water_enable→boiler_start_command'))
  assert.ok(!next.blocks!.some(block => block.name === 'boiler_restart_gate'))
  assert.ok(next.blocks!.some(block => block.name === 'heat_pump_restart_gate'))
})
