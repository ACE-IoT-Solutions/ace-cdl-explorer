import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterStageHold } from '../src/lib/g36WaterStageHoldMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant gains editable source-stage reset holds without losing tuning', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const isHoldBlock = (name: string) => name.startsWith('hot_stage_') || name.startsWith('chilled_stage_')
    || name === 'hot_reset_stage_hold' || name === 'chilled_reset_stage_hold'
    || name.startsWith('hot_reset_held_') || name.startsWith('chilled_reset_held_')
    || name.startsWith('hot_reset_output_') || name.startsWith('chilled_reset_output_')
  plant.blocks = plant.blocks!.filter(block => !isHoldBlock(block.name))
  plant.connections = plant.connections!.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => isHoldBlock(endpoint.split('.')[0])))
  for (const loop of ['hot', 'chilled']) {
    plant.connections.push({ from: `${loop}_reset_limit.y`, to: `${loop}_reset_enabled.u1` })
    plant.connections.push({ from: `${loop}_water_pump_needed.y`, to: `${loop}_reset_enabled.u3` })
    plant.connections.push({ from: `${loop}_reset_previous.y`, to: `${loop}_water_reset_fraction` })
    plant.connections.push({ from: `${loop}_reset_previous.y`, to: `${loop}_dp_reset_limited.u1` })
  }
  plant.connections.push({ from: 'hot_reset_previous.y', to: 'hot_reset_inverse.u2' })
  plant.connections.push({ from: 'chilled_reset_previous.y', to: 'chilled_reset_temperature_excess.u1' })
  plant.blocks.find(block => block.name === 'chilled_reset_trim')!.parameters![0].value = -0.03
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: { ...defaultScenario }, layout: { plant: { x: 12, y: 30 } } }
  const migrated = migrateG36WaterStageHold(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'chilled_reset_trim')!.parameters![0].value, -0.03)
  assert.equal(next.blocks!.find(block => block.name === 'chilled_stage_hold_delay')!.parameters![0].value, 900)
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_stage_hold.y→chilled_reset_enabled.u1'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_output_active.y→chilled_water_reset_fraction'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_output_active.y→chilled_dp_reset_limited.u1'))
  assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'freeze_chilled_water_pump_enable.y→chilled_reset_enabled.u3'))
  assert.ok(!next.connections!.some(edge => edgeEndpoints(edge).join('→') === 'chilled_reset_limit.y→chilled_reset_enabled.u1'))
  assert.deepEqual(migrated.layout.plant, { x: 12, y: 30 })
  assert.equal(migrateG36WaterStageHold(migrated, reference), migrated)
})
