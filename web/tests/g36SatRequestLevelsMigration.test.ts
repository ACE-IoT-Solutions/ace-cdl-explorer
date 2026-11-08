import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SatRequestLevels, needsG36SatRequestLevelsMigration } from '../src/lib/g36SatRequestLevelsMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const former = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block

test('saved binary AHU SAT requests become editable zone request levels without losing reset tuning', () => {
  const old = structuredClone(former)
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'sat_trim')!.parameters![0].value = 0.17
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'cool_setpoint')!.parameters![0].value = 25.5
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36SatRequestLevelsMigration(saved))
  const next = migrateG36SatRequestLevels(saved, reference)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'sat_trim')!.parameters![0].value, 0.17)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'cool_setpoint')!.parameters![0].value, 25.5)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks!.find(block => block.name === name)!
    const ahuName = `ahu_${Math.floor((index - 1) / 5) + 1}`
    const ahu = next.graph.blocks!.find(block => block.name === ahuName)!
    assert.ok(zone.blocks!.some(block => block.name === 'sat_request_three'))
    assert.ok(zone.outputs!.some(port => port.name === 'cooling_sat_reset_request'))
    assert.ok(ahu.inputs!.some(port => port.name === `${name}_sat_request`))
    assert.ok(!ahu.blocks!.some(block => block.name === `${name}_sat_request_count`))
    assert.equal(next.graph.connections!.filter(edge =>
      edgeEndpoints(edge)[1] === `${ahuName}.${name}_sat_request`).length, 1)
    assert.equal(ahu.connections!.filter(edge =>
      edgeEndpoints(edge)[0] === `${name}_sat_request`).length, 1)
  }
  assert.equal(needsG36SatRequestLevelsMigration(next), false)
  assert.equal(migrateG36SatRequestLevels(next, reference), next)
})
