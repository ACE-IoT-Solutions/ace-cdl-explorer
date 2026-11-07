import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FreezeStageOne } from '../src/lib/g36FreezeStageOneMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain timed freeze protection and plant requests without losing edits', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.blocks = ahu.blocks?.filter(block => !block.name.startsWith('freeze_'))
    ahu.outputs = ahu.outputs?.filter(port => !port.name.startsWith('freeze_'))
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('freeze_') && !to.startsWith('freeze_')
    })
    ahu.connections?.push({ from: 'minimum_oa_command.y', to: 'outdoor_air_fraction' })
    ahu.connections?.push({ from: 'heating_coil_needed.y', to: 'heating_coil_enable.u3' })
  }
  const plant = old.blocks?.find(block => block.name === 'plant')!
  plant.inputs = plant.inputs?.filter(port => !port.name.includes('freeze_'))
  plant.blocks = plant.blocks?.filter(block => !block.name.startsWith('freeze_') && block.name !== 'hot_reset_with_freeze_requests')
  plant.connections = plant.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !from.includes('freeze_') && !to.includes('freeze_')
  })
  plant.connections?.push({ from: 'hot_reset_all_request_count.y', to: 'hot_reset_request_real.u' })
  for (const target of ['hot_water_enable', 'hot_water_pump_select.u3', 'hot_reset_enabled.u3']) {
    plant.connections?.push({ from: 'hot_water_pump_needed.y', to: target })
  }
  plant.connections?.push({ from: 'hot_reset_supply_sum.y', to: 'hot_water_supply_setpoint_c' })
  old.outputs = old.outputs?.filter(port => !port.name.includes('freeze_'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.includes('freeze_')))
  const fanGain = old.blocks?.find(block => block.name === 'ahu_1')?.blocks?.find(block => block.name === 'fan_gain')!
  fanGain.parameters![0].value = 0.31
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 42, y: 57 } } }
  const migrated = migrateG36FreezeStageOne(saved, reference)
  const ahu = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  const updatedPlant = migrated.graph.blocks?.find(block => block.name === 'plant')!
  assert.equal(ahu.blocks?.find(block => block.name === 'fan_gain')?.parameters?.[0].value, 0.31)
  assert.ok(ahu.blocks?.some(block => block.name === 'freeze_stage_one_latch'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'freeze_stage_one_outdoor_air.y→outdoor_air_fraction'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'freeze_stage_one_heating_enable.y→heating_coil_enable.u3'))
  assert.ok(updatedPlant.connections?.some(edge => edgeEndpoints(edge).join('→') === 'hot_reset_with_freeze_requests.y→hot_reset_request_real.u'))
  assert.ok(updatedPlant.connections?.some(edge => edgeEndpoints(edge).join('→') === 'freeze_hot_water_pump_enable.y→hot_water_enable'))
  assert.ok(updatedPlant.connections?.some(edge => edgeEndpoints(edge).join('→') === 'freeze_hot_water_target.y→hot_water_supply_setpoint_c'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'ahu_1.freeze_hot_water_request→plant.ahu_1_freeze_hot_water_request'))
  assert.deepEqual(migrated.layout.ahu_1, { x: 42, y: 57 })
  assert.equal(migrateG36FreezeStageOne(migrated, reference), migrated)
})
