import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterPressure } from '../src/lib/g36WaterPressureMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const example = (name: string) => JSON.parse(readFileSync(new URL(`../public/examples/${name}.json`, import.meta.url), 'utf8')) as Block
const reference = example('four_pipe_g36')
const conventional = example('four_pipe_building')

test('saved G36 plant gains pressure feedback while retaining pump minimums', () => {
  const old = structuredClone(reference)
  const plant = old.blocks?.find(block => block.name === 'plant')!
  const conventionalPlant = conventional.blocks?.find(block => block.name === 'plant')!
  const isPressure = (name: string) => name.startsWith('hot_dp_') || name.startsWith('chilled_dp_')
  old.inputs = old.inputs?.filter(port => !port.name.endsWith('_differential_pressure_pa'))
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_dp_setpoint_pa'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.includes('_differential_pressure_pa') || end.endsWith('_dp_setpoint_pa')))
  plant.inputs = plant.inputs?.filter(port => !port.name.endsWith('_differential_pressure_pa'))
  plant.outputs = plant.outputs?.filter(port => !port.name.endsWith('_dp_setpoint_pa'))
  plant.blocks = plant.blocks?.filter(block => !isPressure(block.name))
  plant.connections = plant.connections?.filter(edge => !edgeEndpoints(edge).some(end => isPressure(end.split('.')[0]) || end.endsWith('_dp_setpoint_pa')))
  for (const water of ['hot_water', 'chilled_water']) {
    for (const name of [`${water}_pump_gain`, `${water}_pump_limit`]) {
      plant.blocks?.push(structuredClone(conventionalPlant.blocks?.find(block => block.name === name)!))
    }
    for (const edge of conventionalPlant.connections ?? []) {
      if (edgeEndpoints(edge).some(end => end.startsWith(`${water}_pump_gain.`) || end.startsWith(`${water}_pump_limit.`))) {
        plant.connections?.push(structuredClone(edge))
      }
    }
  }
  plant.blocks!.find(block => block.name === 'hot_water_pump_limit')!.parameters!.find(parameter => parameter.name === 'uMin')!.value = 0.22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 18, y: 24 } } }
  const migrated = migrateG36WaterPressure(saved, reference)
  const next = migrated.graph.blocks?.find(block => block.name === 'plant')!
  assert.equal(next.blocks?.find(block => block.name === 'hot_dp_pump_minimum')?.parameters?.[0].value, 0.22)
  assert.ok(next.blocks?.some(block => block.name === 'chilled_dp_pi'))
  assert.ok(!next.blocks?.some(block => block.name === 'hot_water_pump_gain'))
  assert.ok(next.connections?.some(edge => edgeEndpoints(edge).join('→') === 'hot_dp_speed_minimum.y→hot_water_pump_select.u1'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'hot_water_differential_pressure_pa→plant.hot_water_differential_pressure_pa'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'plant.chilled_dp_setpoint_pa→plant_chilled_dp_setpoint_pa'))
  assert.deepEqual(migrated.layout.plant, { x: 18, y: 24 })
  assert.equal(migrateG36WaterPressure(migrated, reference), migrated)
})
