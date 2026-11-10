import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterPumpStatus } from '../src/lib/g36WaterPumpStatusMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const key = (edge: unknown) => edgeEndpoints(edge as Parameters<typeof edgeEndpoints>[0]).join('→')

test('saved G36 source proof gains motor status while preserving flow thresholds and layout', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  for (const loop of ['hot', 'chilled']) {
    const signal = `${loop}_water_pump_running`
    const andBlock = `${loop}_source_motor_and_flow`
    old.inputs = old.inputs?.filter(port => port.name !== signal)
    old.connections = old.connections?.filter(edge => key(edge) !== `${signal}→plant.${signal}`)
    plant.inputs = plant.inputs?.filter(port => port.name !== signal)
    plant.blocks = plant.blocks?.filter(block => block.name !== andBlock)
    plant.connections = plant.connections?.filter(edge => !key(edge).includes(andBlock) && !key(edge).startsWith(`${signal}→`))
    plant.connections!.push({ from: `${loop}_source_flow_present.y`, to: `${loop}_source_proven.u2` })
  }
  plant.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value = 0.25
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { plant: { x: 21, y: 42 } } }
  const migrated = migrateG36WaterPumpStatus(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.blocks!.find(block => block.name === 'hot_source_flow_threshold')!.parameters![0].value, 0.25)
  assert.equal(upgraded.connections!.some(edge => key(edge) === 'hot_source_motor_and_flow.y→hot_source_proven.u2'), true)
  assert.equal(upgraded.connections!.some(edge => key(edge) === 'chilled_water_pump_running→chilled_source_motor_and_flow.u2'), true)
  assert.equal(migrated.graph.connections!.some(edge => key(edge) === 'hot_water_pump_running→plant.hot_water_pump_running'), true)
  assert.deepEqual(migrated.layout.plant, { x: 21, y: 42 })
  assert.equal(migrateG36WaterPumpStatus(migrated, reference), migrated)
})

test('a custom source-proof input is left wired as edited', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  old.inputs = old.inputs?.filter(port => port.name !== 'hot_water_pump_running')
  old.connections = old.connections?.filter(edge => key(edge) !== 'hot_water_pump_running→plant.hot_water_pump_running')
  plant.inputs = plant.inputs?.filter(port => port.name !== 'hot_water_pump_running')
  plant.blocks = plant.blocks?.filter(block => block.name !== 'hot_source_motor_and_flow')
  plant.connections = plant.connections?.filter(edge => !key(edge).includes('hot_source_motor_and_flow') && !key(edge).startsWith('hot_water_pump_running→'))
  plant.connections!.push({ from: 'freeze_hot_water_pump_enable.y', to: 'hot_source_proven.u2' })
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: {} }
  const migrated = migrateG36WaterPumpStatus(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.connections!.some(edge => key(edge) === 'freeze_hot_water_pump_enable.y→hot_source_proven.u2'), true)
  assert.equal(upgraded.blocks!.some(block => block.name === 'hot_source_motor_and_flow'), false)
  assert.equal(migrated.graph.inputs!.some(port => port.name === 'hot_water_pump_running'), true)
})
