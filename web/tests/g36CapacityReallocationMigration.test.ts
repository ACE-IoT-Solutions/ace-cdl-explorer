import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36CapacityReallocation, needsG36CapacityReallocation, reallocationBlockNames } from '../src/lib/g36CapacityReallocationMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 plant reuses stranded heat-pump capacity without replacing tuned blocks', () => {
  const graph = structuredClone(reference)
  graph.name = 'RenamedG36Sequence'
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  plant.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value = 0.37
  plant.blocks!.find(block => block.name === 'heat_pump_heating_service_cap')!.notes = 'My measured heating limit'
  const added = new Set(reallocationBlockNames)
  plant.blocks = plant.blocks!.filter(block => !added.has(block.name))
  plant.connections = plant.connections!.map(edge => {
    const [from, to] = edgeEndpoints(edge)
    if (from === 'heat_pump_heating_final.y') return {from: 'heat_pump_heating_service_cap.y', to}
    if (from === 'heat_pump_cooling_final.y') return {from: 'heat_pump_cooling_service_cap.y', to}
    return edge
  }).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
  })
  const saved: Project = {version:1, name:graph.name, graph, scenario:structuredClone(defaultScenario), layout:{plant:{x:42,y:24}}}
  assert.equal(needsG36CapacityReallocation(saved), true)
  const migrated = migrateG36CapacityReallocation(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.37)
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_heating_service_cap')!.notes, 'My measured heating limit')
  assert.deepEqual(migrated.layout.plant, {x:42,y:24})
  for (const name of reallocationBlockNames) assert.ok(next.blocks!.some(block => block.name === name), name)
  for (const [service, residual] of [['heating','boiler_residual'],['cooling','chiller_residual']]) {
    const edges = next.connections!.map(edge => edgeEndpoints(edge).join('→'))
    assert.ok(edges.includes(`heat_pump_${service}_final.y→${residual}.u2`))
    assert.ok(!edges.includes(`heat_pump_${service}_service_cap.y→${residual}.u2`))
  }
  assert.equal(needsG36CapacityReallocation(migrated), false)
  assert.equal(migrateG36CapacityReallocation(migrated, reference), migrated)
  const edited = structuredClone(migrated)
  const editedPlant = edited.graph.blocks!.find(block => block.name === 'plant')!
  editedPlant.connections!.find(edge => edgeEndpoints(edge)[1] === 'boiler_residual.u2')!.from = 'heat_pump_zero_capacity.y'
  assert.equal(needsG36CapacityReallocation(edited), false)
  assert.equal(migrateG36CapacityReallocation(edited, reference), edited)
})
