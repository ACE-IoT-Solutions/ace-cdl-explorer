import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateWaterEnergyControl } from '../src/lib/waterEnergyMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  test(`${file} saved plant gains CDL production feedback without losing edits`, () => {
    const reference = JSON.parse(readFileSync(new URL(`../public/examples/${file}`, import.meta.url), 'utf8')) as Block
    const old = structuredClone(reference)
    const plant = old.blocks!.find(block => block.name === 'plant')!
    const sourceNames = new Set(['source_zero', 'hot_water_source_error', 'chilled_water_source_error',
      'hot_water_source_gain', 'chilled_water_source_gain',
      'hot_water_source_sum', 'chilled_water_source_sum', 'hot_water_source_nonnegative', 'chilled_water_source_nonnegative'])
    plant.blocks = plant.blocks!.filter(block => !sourceNames.has(block.name))
    plant.connections = plant.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !sourceNames.has(from.split('.')[0]) && !sourceNames.has(to.split('.')[0])
    })
    for (const [from, to] of [
      ['heat_request_kw', 'heat_pump_heating.u1'], ['heat_request_kw', 'boiler.u1'],
      ['cool_request_kw', 'heat_pump_cooling.u1'], ['cool_request_kw', 'chiller.u1'],
    ]) plant.connections.push({ from, to })
    plant.notes = 'My edited plant notes'
    plant.blocks.find(block => block.name === 'one')!.parameters![0].value = 0.8
    const saved: Project = { version: 1, name: old.name, graph: old, scenario: {} as Project['scenario'], layout: layoutFor(old) }
    const migrated = migrateWaterEnergyControl(saved, reference)
    const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
    assert.equal(upgraded.notes, 'My edited plant notes')
    assert.equal(upgraded.blocks!.find(block => block.name === 'one')!.parameters![0].value, 0.8)
    assert.ok(upgraded.blocks!.some(block => block.name === 'hot_water_source_gain'))
    assert.ok(upgraded.connections!.some(edge => edgeEndpoints(edge).join('|') === 'hot_water_source_nonnegative.y|boiler.u1'))
    assert.equal(upgraded.connections!.filter(edge => edgeEndpoints(edge)[1] === 'boiler.u1').length, 1)
    if (file.includes('g36')) {
      assert.ok(upgraded.connections!.some(edge => edgeEndpoints(edge).join('|') === 'chilled_reset_supply_sum.y|chilled_water_source_error.u2'))
    }
    assert.equal(migrateWaterEnergyControl(migrated, reference), migrated)
  })
}
