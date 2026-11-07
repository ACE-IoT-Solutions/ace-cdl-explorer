import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalHotRequests } from '../src/lib/g36TerminalHotRequestMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 hot reset gains VAV requests and keeps its edited reset span', () => {
  const old = structuredClone(reference)
  const plant = old.blocks!.find(block => block.name === 'plant')!
  const added = (name: string) => name.startsWith('terminal_') || /^zone_\d\d_hot_/.test(name) || name === 'hot_reset_all_request_count'
  plant.inputs = plant.inputs!.filter(port => !/^zone_\d\d_(reheat_valve|heating_demand)$/.test(port.name))
  plant.outputs = plant.outputs!.filter(port => port.name !== 'terminal_hot_water_request_count')
  plant.blocks = plant.blocks!.filter(block => !added(block.name))
  plant.connections = plant.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(added(from.split('.')[0]) || added(to.split('.')[0]) || to === 'terminal_hot_water_request_count')
  })
  plant.connections.push({ from: 'hot_reset_request_count.y', to: 'hot_reset_request_real.u' })
  old.outputs = old.outputs!.filter(port => port.name !== 'plant_terminal_hot_water_request_count')
  old.connections = old.connections!.filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !/^plant.zone_\d\d_(reheat_valve|heating_demand)$/.test(to) && to !== 'plant_terminal_hot_water_request_count'
  })
  plant.notes = 'My edited plant notes'
  plant.blocks.find(block => block.name === 'hot_reset_warm_delta')!.parameters![0].value = -7.0
  const saved: Project = { version: 1, name: old.name, graph: old, layout: layoutFor(old), scenario: {} as Project['scenario'] }
  const migrated = migrateG36TerminalHotRequests(saved, reference)
  const upgraded = migrated.graph.blocks!.find(block => block.name === 'plant')!
  assert.equal(upgraded.notes, 'My edited plant notes')
  assert.equal(upgraded.blocks!.find(block => block.name === 'hot_reset_warm_delta')!.parameters![0].value, -7.0)
  assert.ok(upgraded.blocks!.some(block => block.name === 'zone_01_hot_request_delay'))
  assert.equal(upgraded.connections!.filter(edge => edgeEndpoints(edge)[1] === 'hot_reset_request_real.u').length, 1)
  assert.equal(migrated.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'plant.zone_01_heating_demand').length, 1)
  assert.equal(migrateG36TerminalHotRequests(migrated, reference), migrated)
})
