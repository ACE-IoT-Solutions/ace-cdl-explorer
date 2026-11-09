import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SatSampledSetpoint, needsG36SatSampledSetpointMigration } from '../src/lib/g36SatSampledSetpointMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const rewiredTargets = ['sat_previous_cooling_setpoint.u', 'sat_setpoint_rise.u1', 'sat_setpoint_fall.u2']

test('saved SAT suppression compares sampled setpoints and preserves the delayed sample tuning', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(child => child.name === 'zone_01')!
  zone.blocks = zone.blocks!.filter(child => child.name !== 'sat_sampled_cooling_setpoint')
  zone.connections = zone.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return from !== 'sat_sampled_cooling_setpoint.y' && to !== 'sat_sampled_cooling_setpoint.u'
  })
  for (const target of rewiredTargets) zone.connections.push({ from: 'window_cool_target.y', to: target })
  zone.blocks.find(child => child.name === 'sat_previous_cooling_setpoint')!.parameters![0].value = 180
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 18, y: 34 } } }
  assert.ok(needsG36SatSampledSetpointMigration(saved))
  const next = migrateG36SatSampledSetpoint(saved, reference)
  const updated = next.graph.blocks!.find(child => child.name === 'zone_01')!
  assert.deepEqual(next.layout.zone_01, { x: 18, y: 34 })
  assert.equal(updated.blocks!.find(child => child.name === 'sat_previous_cooling_setpoint')!.parameters![0].value, 180)
  assert.equal(updated.blocks!.find(child => child.name === 'sat_sampled_cooling_setpoint')!.parameters![0].value, 120)
  for (const target of rewiredTargets) {
    assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
      .map(edge => edgeEndpoints(edge)[0]), ['sat_sampled_cooling_setpoint.y'])
  }
  assert.equal(needsG36SatSampledSetpointMigration(next), false)
  assert.equal(migrateG36SatSampledSetpoint(next, reference), next)
})
