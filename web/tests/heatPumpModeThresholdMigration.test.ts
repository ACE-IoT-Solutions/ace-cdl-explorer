import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateHeatPumpModeThreshold, needsHeatPumpModeThresholdMigration } from '../src/lib/heatPumpModeThresholdMigration'
import type { Block, Project } from '../src/lib/types'

for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  test(`${file} saved mode threshold migrates without replacing other tuning`, () => {
    const graph = JSON.parse(readFileSync(new URL(`../public/examples/${file}`, import.meta.url), 'utf8')) as Block
    const saved: Project = { version: 1, name: graph.name, graph, scenario: {} as Project['scenario'], layout: {} }
    const plant = saved.graph.blocks?.find(block => block.name === 'plant')
    const threshold = plant?.blocks?.find(block => block.name === 'heat_pump_mode_threshold')
    const parameter = threshold?.parameters?.find(item => item.name === 'k')
    assert.ok(parameter)
    assert.equal(parameter.value, 0)
    parameter.value = 0.5
    const other = plant?.blocks?.find(block => block.name === 'heat_pump_mode_cooling_weight')
    assert.ok(other)
    other.notes = 'Custom note'
    assert.equal(needsHeatPumpModeThresholdMigration(saved), true)
    const migrated = migrateHeatPumpModeThreshold(saved)
    assert.notEqual(migrated, saved)
    assert.equal(parameter.value, 0.5)
    assert.equal(migrated.graph.blocks?.find(block => block.name === 'plant')?.blocks?.find(block => block.name === 'heat_pump_mode_threshold')?.parameters?.find(item => item.name === 'k')?.value, 0)
    assert.equal(migrated.graph.blocks?.find(block => block.name === 'plant')?.blocks?.find(block => block.name === 'heat_pump_mode_cooling_weight')?.notes, 'Custom note')
    assert.equal(migrateHeatPumpModeThreshold(migrated), migrated)
  })
}
