import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SupplyTemperatureModes, needsG36SupplyTemperatureModeMigration } from '../src/lib/g36SupplyTemperatureModeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved AHU SAT reset gains group mode selection while keeping edited parameters', () => {
  const graph = structuredClone(reference)
  for (const index of [1, 2]) {
    const ahu = graph.blocks!.find(block => block.name === `ahu_${index}`)!
    ahu.inputs = ahu.inputs!.filter(port => port.name !== 'operation_mode')
    ahu.blocks = ahu.blocks!.filter(block => !block.name.startsWith('sat_mode_'))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('sat_mode_') && !to.startsWith('sat_mode_')
    })
    const template = reference.blocks!.find(block => block.name === `ahu_${index}`)!
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'sat_mode_fan_select.y' && !to.startsWith('sat_mode_'))
        ahu.connections.push({ from: 'sat_output.y', to })
    }
    ahu.connections.push(
      { from: 'heating_coil_needed.y', to: 'freeze_stage_one_heating_enable.u1' },
      { from: 'cooling_coil_needed.y', to: 'cooling_coil_enable.u3' },
    )
  }
  graph.connections = graph.connections!.filter(edge =>
    !/^ahu_[12]\.operation_mode$/.test(edgeEndpoints(edge)[1]))
  graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'sat_minimum')!.parameters![0].value = 13.5
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 19, y: 27 } } }
  assert.ok(needsG36SupplyTemperatureModeMigration(saved))
  const next = migrateG36SupplyTemperatureModes(saved, reference)
  const ahu = next.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.deepEqual(next.layout.ahu_1, { x: 19, y: 27 })
  assert.equal(ahu.blocks!.find(block => block.name === 'sat_minimum')!.parameters![0].value, 13.5)
  assert.ok(ahu.blocks!.some(block => block.name === 'sat_mode_fan_select'))
  assert.deepEqual(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'supply_air_temp')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_mode_fan_select.y'])
  assert.deepEqual(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'freeze_stage_one_heating_enable.u1')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_mode_heating_permitted.y'])
  assert.deepEqual(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'cooling_coil_enable.u3')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_mode_cooling_permitted.y'])
  assert.ok(next.graph.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'zone_group_1.operation_mode→ahu_1.operation_mode'))
  assert.equal(needsG36SupplyTemperatureModeMigration(next), false)
  assert.equal(migrateG36SupplyTemperatureModes(next, reference), next)
})
