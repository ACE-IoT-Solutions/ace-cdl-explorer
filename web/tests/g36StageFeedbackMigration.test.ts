import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36StageFeedback } from '../src/lib/g36StageFeedbackMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 reset holds gain physical running proof and retain edited hold time', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const added = /^(hot|chilled)_stage_(hp_confirmed|backup_confirmed|hp_not_requested|backup_not_requested|hp_ready|backup_ready|sources_ready|ready_integer|ready_real|ready_observed|ready_threshold|ready_confirmed|ready_and_flow)$/
  plant.blocks = plant.blocks!.filter(block => !added.test(block.name))
  plant.blocks.find(block => block.name === 'hot_stage_hold_delay')!.parameters!.find(parameter => parameter.name === 'delayTime')!.value = 1200
  old.inputs = old.inputs!.filter(port => !port.name.endsWith('_running'))
  plant.inputs = plant.inputs!.filter(port => !port.name.endsWith('_running'))
  old.connections = old.connections!.filter(edge => !edgeEndpoints(edge)[0].endsWith('_running'))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !added.test(from.split('.')[0]) && !added.test(to.split('.')[0])
  })
  for (const loop of ['hot', 'chilled']) {
    plant.connections.push(
      {from:`${loop}_stage_hp_active.y`,to:`${loop}_stage_hp_select.u3`},
      {from:`${loop}_stage_backup_active.y`,to:`${loop}_stage_backup_select.u3`},
      {from:`${loop}_stage_stable_and_proven.y`,to:`${loop}_stage_hold_delay.u`},
    )
  }
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { plant: { x: 42, y: 24 } } }
  const migrated = migrateG36StageFeedback(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'hot_stage_hold_delay')!.parameters!.find(parameter => parameter.name === 'delayTime')!.value, 1200)
  for (const loop of ['hot', 'chilled']) {
    for (const target of [`${loop}_stage_hp_select.u3`, `${loop}_stage_backup_select.u3`, `${loop}_stage_hold_delay.u`]) {
      assert.equal(next.connections!.filter(edge => edgeEndpoints(edge)[1] === target).length, 1, target)
    }
    assert.ok(next.blocks!.some(block => block.name === `${loop}_stage_ready_observed`))
  }
  assert.deepEqual(migrated.layout.plant, { x: 42, y: 24 })
  assert.equal(migrateG36StageFeedback(migrated, reference), migrated)
})
