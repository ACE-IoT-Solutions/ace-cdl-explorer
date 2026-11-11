import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36WaterCapacity, needsG36WaterCapacityMigration } from '../src/lib/g36WaterCapacityMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 dispatch gains measured service caps without replacing tuned CDL', () => {
  const graph = structuredClone(reference)
  graph.name = 'RenamedG36Sequence'
  const plant = graph.blocks!.find(block => block.name === 'plant')!
  plant.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value = 0.37
  for (const service of ['heating', 'cooling']) {
    const capacity = `heat_pump_${service}_available_capacity_kw`
    const limiter = `heat_pump_${service}_service_cap`
    graph.inputs = graph.inputs!.filter(port => port.name !== capacity)
    graph.connections = graph.connections!.filter(edge => !edgeEndpoints(edge).includes(capacity))
    plant.inputs = plant.inputs!.filter(port => port.name !== capacity)
    plant.blocks = plant.blocks!.filter(block => block.name !== limiter)
    plant.connections = plant.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from !== capacity && to !== `${limiter}.u1` && to !== `${limiter}.u2`
    }).map(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === `${limiter}.y` ? {from: `heat_pump_${service}.y`, to} : edge
    })
  }
  const saved: Project = {version:1, name:graph.name, graph, scenario:structuredClone(defaultScenario), layout:{plant:{x:42,y:24}}}
  const migrated = migrateG36WaterCapacity(saved, reference)
  const next = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(next.blocks!.find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.37)
  assert.deepEqual(migrated.layout.plant, {x:42,y:24})
  for (const service of ['heating', 'cooling']) {
    const capacity = `heat_pump_${service}_available_capacity_kw`
    const limiter = `heat_pump_${service}_service_cap`
    assert.ok(migrated.graph.connections!.some(edge => edgeEndpoints(edge).join('→') === `${capacity}→plant.${capacity}`))
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === `heat_pump_${service}.y→${limiter}.u1`))
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === `${capacity}→${limiter}.u2`))
    assert.ok(next.connections!.some(edge => edgeEndpoints(edge).join('→') === `${limiter}.y→${service === 'heating' ? 'boiler' : 'chiller'}_residual.u2`))
  }
  assert.equal(migrateG36WaterCapacity(migrated, reference), migrated)
  const partial = structuredClone(migrated)
  partial.graph.connections = partial.graph.connections!.filter(edge =>
    edgeEndpoints(edge).join('→') !== 'heat_pump_heating_available_capacity_kw→plant.heat_pump_heating_available_capacity_kw')
  assert.equal(needsG36WaterCapacityMigration(partial), true)
  const repaired = migrateG36WaterCapacity(partial, reference)
  assert.ok(repaired.graph.connections!.some(edge =>
    edgeEndpoints(edge).join('→') === 'heat_pump_heating_available_capacity_kw→plant.heat_pump_heating_available_capacity_kw'))
  assert.equal(repaired.graph.blocks!.find(block => block.name === 'plant')!.blocks!
    .find(block => block.name === 'heat_pump_utilization_limit')!.parameters![0].value, 0.37)
})
