import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36BackupStage, needsG36BackupStageMigration } from '../src/lib/g36BackupStageMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

function prior(): Project {
  const graph = structuredClone(reference)
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  for (const source of ['boiler', 'chiller']) {
    const delay = `${source}_stage_up_delay`
    plant.blocks = plant.blocks!.filter(block => block.name !== delay)
    plant.connections = plant.connections!.filter(edge => edgeEndpoints(edge)[1] !== `${delay}.u`)
    plant.connections.find(edge => edgeEndpoints(edge)[1] === `${source}_minimum_run.u`)!.from = `${source}_start_positive.y`
  }
  plant.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'trueHoldDuration')!.value = 420
  for (const source of ['boiler','chiller']) plant.blocks!.find(block => block.name === `${source}_minimum_run`)!.parameters!
    .find(parameter => parameter.name === 'falseHoldDuration')!.value = 300
  return {version:1, name:graph.name, graph, scenario:structuredClone(defaultScenario), layout:{plant:{x:30,y:40}}}
}

test('saved G36 backup starts gain editable sustained-load qualification', () => {
  const saved = prior()
  assert.equal(needsG36BackupStageMigration(saved), true)
  const upgraded = migrateG36BackupStage(saved, reference)
  const plant = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.deepEqual(upgraded.layout.plant, {x:30,y:40})
  assert.equal(plant.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'trueHoldDuration')!.value, 420)
  for (const source of ['boiler', 'chiller']) {
    const delay = `${source}_stage_up_delay`
    assert.equal(plant.blocks!.find(block => block.name === delay)!.parameters!
      .find(parameter => parameter.name === 'delayTime')!.value, 300)
    assert.equal(plant.blocks!.find(block => block.name === `${source}_minimum_run`)!.parameters!
      .find(parameter => parameter.name === 'falseHoldDuration')!.value, 0)
    assert.equal(plant.connections!.filter(edge => edgeEndpoints(edge)[1] === `${delay}.u`).length, 1)
    assert.equal(plant.connections!.find(edge => edgeEndpoints(edge)[1] === `${source}_minimum_run.u`)!.from, `${delay}.y`)
  }
  assert.equal(needsG36BackupStageMigration(upgraded), false)
  assert.equal(migrateG36BackupStage(upgraded, reference), upgraded)
})

test('a custom backup start path is preserved', () => {
  const saved = prior()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_minimum_run.u')!.from = 'custom_boiler_stage.y'
  const upgraded = migrateG36BackupStage(saved, reference)
  const next = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_minimum_run.u')!.from, 'custom_boiler_stage.y')
  assert.ok(!next.blocks!.some(block => block.name === 'boiler_stage_up_delay'))
  assert.ok(next.blocks!.some(block => block.name === 'chiller_stage_up_delay'))
})

test('an edited minimum-off duration is retained', () => {
  const saved = prior()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'falseHoldDuration')!.value = 420
  const upgraded = migrateG36BackupStage(saved,reference)
  const next = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'boiler_minimum_run')!.parameters!
    .find(parameter => parameter.name === 'falseHoldDuration')!.value,420)
})

test('a custom final source command does not gain an unused stage branch', () => {
  const saved = prior()
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_start_command')!.from = 'custom_final_command.y'
  const upgraded = migrateG36BackupStage(saved, reference)
  const next = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_start_command')!.from, 'custom_final_command.y')
  assert.ok(!next.blocks!.some(block => block.name === 'boiler_stage_up_delay'))
  assert.ok(next.blocks!.some(block => block.name === 'chiller_stage_up_delay'))
})

test('partial stage-delay wiring is repaired without replacing edited duration', () => {
  const saved = structuredClone({version:1, name:reference.name, graph:reference, scenario:defaultScenario, layout:{}}) as Project
  const plant = saved.graph.blocks!.find(block => block.name === 'plant')!
  plant.connections = plant.connections!.filter(edge => edgeEndpoints(edge)[1] !== 'boiler_stage_up_delay.u')
  plant.blocks!.find(block => block.name === 'boiler_stage_up_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value = 180
  const upgraded = migrateG36BackupStage(saved, reference)
  const next = upgraded.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'boiler_stage_up_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value, 180)
  assert.equal(next.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_stage_up_delay.u')!.from, 'boiler_start_positive.y')
})
