import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SourceMinRun, needsG36SourceMinRunMigration } from '../src/lib/g36SourceMinRunMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const drivers = {
  heat_pump: 'heat_pump_start_or.y',
  boiler: 'boiler_start_positive.y',
  chiller: 'chiller_start_positive.y',
} as const

function legacy(): Project {
  const graph = structuredClone(reference)
  graph.name = 'Edited G36 controls'
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  plant.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value = 8.5
  const added = new Set(Object.keys(drivers).flatMap(source => [
    `${source}_minimum_run`, `${source}_start_fault_clear`,
    `${source}_start_allowed`, `${source}_start_gate`, `${source}_issued_start_real`,
  ]))
  plant.blocks = plant.blocks!.filter(block => !added.has(block.name))
  plant.connections = plant.connections!.filter(edge => {
    const [from,to] = edgeEndpoints(edge)
    return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) && !to.endsWith('_start_command')
  })
  for (const [source,driver] of Object.entries(drivers)) {
    plant.connections.push({from:driver,to:`${source}_start_command`})
  }
  return {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{plant:{x:31,y:47}}}
}

test('saved G36 source starts gain editable minimum run holds without losing tuning', () => {
  const saved = legacy()
  assert.equal(needsG36SourceMinRunMigration(saved),true)
  const migrated = migrateG36SourceMinRun(saved,reference)
  const plant = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(plant.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value,8.5)
  assert.deepEqual(migrated.layout.plant,{x:31,y:47})
  for (const [source,driver] of Object.entries(drivers)) {
    const hold = plant.blocks!.find(block => block.name === `${source}_minimum_run`)!
    assert.equal(hold.parameters!.find(parameter => parameter.name === 'trueHoldDuration')!.value,300)
    assert.equal(hold.parameters!.find(parameter => parameter.name === 'falseHoldDuration')!.value,300)
    assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === `${driver}→${source}_minimum_run.u`))
    assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === `${source}_start_gate.y→${source}_start_command`))
    assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === `${source}_issued_start_real.y→${source}_start_request_observed.u`))
    assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === `${source}_start_fault_latch.y→${source}_start_fault_clear.u`))
    assert.ok(plant.connections!.some(edge => edgeEndpoints(edge).join('→') === `${source}_available→${source}_start_allowed.u1`))
  }
  assert.equal(needsG36SourceMinRunMigration(migrated),false)
  assert.equal(migrateG36SourceMinRun(migrated,reference),migrated)
})

test('earlier minimum-run controls gain off lockout without replacing tuned on duration', () => {
  const graph = structuredClone(reference)
  graph.name = 'Prior G36 source holds'
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  for (const [source,oldDriver] of Object.entries({
    heat_pump:'heat_pump_start_request.y', boiler:'boiler_availability_gate.y', chiller:'chiller_availability_gate.y',
  })) {
    plant.blocks = plant.blocks!.filter(block => block.name !== `${source}_issued_start_real`)
    const hold = plant.blocks!.find(block => block.name === `${source}_minimum_run`)!
    hold.parameters!.find(parameter => parameter.name === 'falseHoldDuration')!.value = 0
    hold.parameters!.find(parameter => parameter.name === 'trueHoldDuration')!.value = source === 'boiler' ? 420 : 300
    plant.connections = plant.connections!.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return from.split('.')[0] !== `${source}_issued_start_real` && to.split('.')[0] !== `${source}_issued_start_real`
        && to !== `${source}_start_request_observed.u`
    })
    plant.connections.push({from:oldDriver,to:`${source}_start_request_observed.u`})
  }
  const saved:Project = {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{plant:{x:8,y:9}}}
  assert.equal(needsG36SourceMinRunMigration(saved),true)
  const migrated = migrateG36SourceMinRun(saved,reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!.find(parameter => parameter.name === 'trueHoldDuration')!.value,420)
  for (const source of Object.keys(drivers)) {
    assert.equal(next.blocks!.find(block => block.name === `${source}_minimum_run`)!.parameters!.find(parameter => parameter.name === 'falseHoldDuration')!.value,300)
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === `${source}_issued_start_real.y→${source}_start_request_observed.u`))
  }
  assert.deepEqual(migrated.layout.plant,{x:8,y:9})
  assert.equal(needsG36SourceMinRunMigration(migrated),false)
})

test('a partially saved issued-start feedback wire is repaired', () => {
  const prior = migrateG36SourceMinRun(legacy(),reference)
  const partial = structuredClone(prior)
  const plant = partial.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections = plant.connections!.filter(edge => edgeEndpoints(edge)[1] !== 'boiler_start_request_observed.u')
  assert.equal(needsG36SourceMinRunMigration(partial),true)
  const repaired = migrateG36SourceMinRun(partial,reference)
  assert.ok(repaired.graph.blocks!.find(block => block.name === 'plant')!.connections!
    .some(edge => edgeEndpoints(edge).join('→') === 'boiler_issued_start_real.y→boiler_start_request_observed.u'))
  assert.equal(needsG36SourceMinRunMigration(repaired),false)
})

test('custom source start wiring is preserved', () => {
  const saved = legacy()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_start_command')!.from = 'hot_water_enable'
  const migrated = migrateG36SourceMinRun(saved,reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'hot_water_enable→boiler_start_command'))
  assert.ok(!next.blocks!.some(block => block.name === 'boiler_minimum_run'))
  assert.ok(next.blocks!.some(block => block.name === 'heat_pump_minimum_run'))
})

test('custom fault qualification is preserved without imposing an unsafe off lockout', () => {
  const saved = legacy()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.push({from:'heat_pump_heating_flow_gate.y',to:'heat_pump_start_request_observed.u'})
  const migrated = migrateG36SourceMinRun(saved,reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'heat_pump_heating_flow_gate.y→heat_pump_start_request_observed.u'))
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'falseHoldDuration')!.value,0)
})

test('conventional controls are unaffected', () => {
  const graph = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block
  const saved: Project = {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{}}
  assert.equal(needsG36SourceMinRunMigration(saved),false)
  assert.equal(migrateG36SourceMinRun(saved,reference),saved)
})
