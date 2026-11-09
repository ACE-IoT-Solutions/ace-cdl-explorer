import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SatCoolingLoop, needsG36SatCoolingLoopMigration } from '../src/lib/g36SatCoolingLoopMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved SAT requests use the cooling PI while retaining request and PI tuning', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(child => child.name === 'zone_01')!
  zone.blocks = zone.blocks!.filter(child => !['sat_cooling_loop_scale', 'sat_cooling_loop_limit'].includes(child.name))
  zone.connections = zone.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !['sat_cooling_loop_scale', 'sat_cooling_loop_limit'].includes(from.split('.')[0]) &&
      !['sat_cooling_loop_scale', 'sat_cooling_loop_limit'].includes(to.split('.')[0])
  })
  zone.connections.push({ from: 'cool_demand_limit.y', to: 'sat_cooling_loop_high.u' })
  zone.blocks.find(child => child.name === 'cool_pi')!.parameters![0].value = 0.72
  zone.blocks.find(child => child.name === 'sat_cooling_loop_high')!.parameters![1].value = 0.92
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 14, y: 29 } } }
  assert.ok(needsG36SatCoolingLoopMigration(saved))
  const next = migrateG36SatCoolingLoop(saved, reference)
  const updated = next.graph.blocks!.find(child => child.name === 'zone_01')!
  assert.deepEqual(next.layout.zone_01, { x: 14, y: 29 })
  assert.equal(updated.blocks!.find(child => child.name === 'cool_pi')!.parameters![0].value, 0.72)
  assert.equal(updated.blocks!.find(child => child.name === 'sat_cooling_loop_high')!.parameters![1].value, 0.92)
  assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sat_cooling_loop_high.u')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_cooling_loop_limit.y'])
  assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sat_cooling_loop_scale.u')
    .map(edge => edgeEndpoints(edge)[0]), ['cool_pi.y'])
  assert.equal(needsG36SatCoolingLoopMigration(next), false)
  assert.equal(migrateG36SatCoolingLoop(next, reference), next)
})
