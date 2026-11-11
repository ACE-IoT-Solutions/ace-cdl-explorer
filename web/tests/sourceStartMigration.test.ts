import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateSourceStartRequests, needsSourceStartMigration } from '../src/lib/sourceStartMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  test(`${file} saved plant gains CDL source starts and keeps tuning`, () => {
    const reference = JSON.parse(readFileSync(new URL(`../public/examples/${file}`, import.meta.url), 'utf8')) as Block
    const graph = structuredClone(reference)
    graph.name = `Edited ${file}`
    const plant = graph.blocks!.find(block => block.name === 'plant')!
    const tuned = plant.blocks!.find(block => block.name === 'hot_water_source_gain')!
    tuned.parameters![0].value = 9.25
    const newBlocks = new Set(['source_start_threshold', 'heat_pump_heat_start_positive',
      'heat_pump_cool_start_positive', 'heat_pump_start_or', 'boiler_start_positive', 'chiller_start_positive'])
    plant.blocks = plant.blocks!.filter(block => !newBlocks.has(block.name))
    plant.outputs = plant.outputs!.filter(port => !port.name.endsWith('_start_command'))
    plant.connections = plant.connections!.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return !newBlocks.has(from.split('.')[0]) && !newBlocks.has(to.split('.')[0]) && !to.endsWith('_start_command')
    })
    graph.outputs = graph.outputs!.filter(port => !port.name.endsWith('_start_command'))
    graph.connections = graph.connections!.filter(edge => !edgeEndpoints(edge)[1].endsWith('_start_command'))
    const saved: Project = {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{plant:{x:37,y:42}}}
    assert.equal(needsSourceStartMigration(saved), true)
    const migrated = migrateSourceStartRequests(saved, reference)
    const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
    assert.equal(next.blocks!.find(block => block.name === 'hot_water_source_gain')!.parameters![0].value, 9.25)
    assert.deepEqual(migrated.layout.plant,{x:37,y:42})
    for (const source of ['heat_pump','boiler','chiller']) {
      const port = `${source}_start_command`
      assert.ok(next.outputs!.some(output => output.name === port && output.type === 'Boolean'))
      assert.ok(migrated.graph.outputs!.some(output => output.name === `plant_${port}` && output.type === 'Boolean'))
      assert.ok(next.connections!.some(edge => edgeEndpoints(edge)[1] === port))
      assert.ok(migrated.graph.connections!.some(edge => edgeEndpoints(edge).join('→') === `plant.${port}→plant_${port}`))
    }
    for (const [detector,output] of [
      ['heat_pump_heat_start_positive','heat_pump_heating_kw'],
      ['heat_pump_cool_start_positive','heat_pump_cooling_kw'],
      ['boiler_start_positive','boiler_kw'],
      ['chiller_start_positive','chiller_kw'],
    ]) {
      const driver = next.connections!.find(edge => edgeEndpoints(edge)[1] === output)!
      const startInput = next.connections!.find(edge => edgeEndpoints(edge)[1] === `${detector}.u1`)!
      assert.equal(edgeEndpoints(startInput)[0],edgeEndpoints(driver)[0])
    }
    assert.equal(needsSourceStartMigration(migrated),false)
    assert.equal(migrateSourceStartRequests(migrated,reference),migrated)
    const partial = structuredClone(migrated)
    partial.graph.connections = partial.graph.connections!.filter(edge =>
      edgeEndpoints(edge)[1] !== 'plant_heat_pump_start_command')
    assert.equal(needsSourceStartMigration(partial),true)
    const repaired = migrateSourceStartRequests(partial,reference)
    assert.ok(repaired.graph.connections!.some(edge =>
      edgeEndpoints(edge).join('→') === 'plant.heat_pump_start_command→plant_heat_pump_start_command'))
    assert.equal(repaired.graph.blocks!.find(block => block.name === 'plant')!.blocks!
      .find(block => block.name === 'hot_water_source_gain')!.parameters![0].value,9.25)
  })
}
