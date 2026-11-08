import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SupplyAirReset } from '../src/lib/g36SupplyAirResetMigration'
import { migrateG36SatRequestLevels } from '../src/lib/g36SatRequestLevelsMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain sampled VAV supply-temperature reset and retain coil edits', () => {
  const old = structuredClone(reference)
  const added = (name: string) => name.startsWith('sat_') || /^zone_\d\d_sat_/.test(name)
  for (const index of [1, 2]) {
    const ahu = old.blocks!.find(block => block.name === `ahu_${index}`)!
    const template = reference.blocks!.find(block => block.name === `ahu_${index}`)!
    ahu.inputs = ahu.inputs!.filter(port => !/^zone_\d\d_sat_request$/.test(port.name))
    ahu.outputs = ahu.outputs!.filter(port => port.name !== 'sat_request_count')
    ahu.blocks = ahu.blocks!.filter(block => !added(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added(from.split('.')[0]) && !added(to.split('.')[0]) && to !== 'sat_request_count'
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'sat_output.y') ahu.connections.push({ from: 'supply_reset.y', to })
    }
    ahu.notes = `My AHU ${index} notes`
    ahu.blocks.find(block => block.name === 'cooling_supply')!.parameters![0].value = 13.0
  }
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_sat_request_count$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !/^ahu_[12]\.zone_\d\d_sat_request$/.test(to) && !/^ahu_[12]_sat_request_count$/.test(to)
  })
  const saved: Project = { version: 1, name: old.name, graph: old, layout: layoutFor(old), scenario: {} as Project['scenario'] }
  const migrated = migrateG36SupplyAirReset(saved, reference)
  const ahu = migrated.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.equal(ahu.notes, 'My AHU 1 notes')
  assert.equal(ahu.blocks!.find(block => block.name === 'cooling_supply')!.parameters![0].value, 13.0)
  assert.ok(ahu.blocks!.some(block => block.name === 'sat_previous'))
  assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'supply_air_temp').length, 1)
  assert.equal(migrated.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'ahu_1.zone_01_sat_request').length, 1)
  assert.equal(migrateG36SupplyAirReset(migrated, reference), migrated)
})

test('an older G36 graph gains the new AHU reset and VAV request branch in sequence', () => {
  const former = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block
  const old = structuredClone(former)
  for (const index of [1, 2]) {
    const ahu = old.blocks!.find(block => block.name === `ahu_${index}`)!
    const retired = new Set(ahu.blocks!.filter(block =>
      block.name.startsWith('sat_') || /^zone_\d\d_sat_/.test(block.name)).map(block => block.name))
    const outgoing = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === 'sat_output.y' && !retired.has(to.split('.')[0])
    })
    ahu.blocks = ahu.blocks!.filter(block => !retired.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !retired.has(from.split('.')[0]) && !retired.has(to.split('.')[0]) && to !== 'sat_request_count'
    })
    for (const edge of outgoing) ahu.connections.push({ from: 'supply_reset.y', to: edgeEndpoints(edge)[1] })
    ahu.inputs = ahu.inputs!.filter(port => !/^zone_\d\d_(cooling_demand|damper_position)$/.test(port.name))
    ahu.outputs = ahu.outputs!.filter(port => port.name !== 'sat_request_count')
  }
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_sat_request_count$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !/^ahu_[12]\.zone_\d\d_(cooling_demand|damper_position)$/.test(to) &&
      !/^ahu_[12]_sat_request_count$/.test(to)
  })
  const saved: Project = { version: 1, name: old.name, graph: old, layout: {}, scenario: {} as Project['scenario'] }
  const withAhu = migrateG36SupplyAirReset(saved, reference)
  const complete = migrateG36SatRequestLevels(withAhu, reference)
  const ahu = complete.graph.blocks!.find(block => block.name === 'ahu_1')!
  const zone = complete.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.ok(ahu.blocks!.some(block => block.name === 'sat_previous'))
  assert.ok(ahu.inputs!.some(port => port.name === 'zone_01_sat_request'))
  assert.ok(zone.outputs!.some(port => port.name === 'cooling_sat_reset_request'))
  assert.equal(complete.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'ahu_1.zone_01_sat_request').length, 1)
  assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'supply_air_temp').length, 1)
})
