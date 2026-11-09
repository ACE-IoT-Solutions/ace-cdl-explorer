import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ObcSupplyAirReset, needsG36ObcSupplyAirResetMigration } from '../src/lib/g36ObcSupplyAirResetMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 weather reset becomes sampled OBC Line while retaining tuned maximum and layout', () => {
  const graph = structuredClone(reference)
  for (const index of [1, 2]) {
    const ahu = graph.blocks!.find(block => block.name === `ahu_${index}`)!
    ahu.blocks = ahu.blocks!.filter(block => !['sat_maximum', 'sat_outdoor_low', 'sat_outdoor_high',
      'sat_request_sample', 'sat_weather_line'].includes(block.name))
    ahu.blocks.push({ name: 'warmup_supply', block_type: 'Constant',
      inputs: [], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 17.5 }] })
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !['sat_maximum', 'sat_outdoor_low', 'sat_outdoor_high', 'sat_request_sample',
        'sat_weather_line'].includes(from.split('.')[0]) &&
        !['sat_maximum', 'sat_outdoor_low', 'sat_outdoor_high', 'sat_request_sample',
          'sat_weather_line'].includes(to.split('.')[0])
    })
    ahu.connections.push(
      { from: 'sat_request_real.y', to: 'sat_excess_requests.u1' },
      { from: 'fan_requested.y', to: 'sat_fan_delay.u' },
      { from: 'sat_previous.y', to: 'sat_output.u1' },
      { from: 'sat_output.y', to: 'sat_mode_weather_select.u1' },
    )
    ahu.blocks.find(block => block.name === 'sat_minimum')!.parameters![0].value = 14
    ahu.blocks.find(block => block.name === 'sat_previous')!.parameters!
      .find(parameter => parameter.name === 'y_start')!.value = 16
    ahu.blocks.find(block => block.name === 'sat_fan_delay')!.parameters!
      .find(parameter => parameter.name === 'delayTime')!.value = 600
  }
  const saved: Project = { version: 1, name: graph.name, graph, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 43, y: 76 } } }
  assert.ok(needsG36ObcSupplyAirResetMigration(saved))
  const next = migrateG36ObcSupplyAirReset(saved, reference)
  const ahu = next.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.deepEqual(next.layout.ahu_1, { x: 43, y: 76 })
  assert.equal(ahu.blocks!.find(block => block.name === 'sat_maximum')!.parameters![0].value, 17.5)
  assert.equal(ahu.blocks!.find(block => block.name === 'sat_minimum')!.parameters![0].value, 12)
  assert.equal(ahu.blocks!.find(block => block.name === 'sat_previous')!.parameters!
    .find(parameter => parameter.name === 'y_start')!.value, 18)
  assert.deepEqual(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sat_mode_weather_select.u1')
    .map(edge => edgeEndpoints(edge)[0]), ['sat_weather_line.y'])
  assert.ok(!ahu.blocks!.some(block => block.name === 'warmup_supply'))
  assert.equal(needsG36ObcSupplyAirResetMigration(next), false)
  assert.equal(migrateG36ObcSupplyAirReset(next, reference), next)
})

test('partly upgraded saved graph removes legacy weather cards without duplicating Line', () => {
  const graph = structuredClone(reference)
  for (const index of [1, 2]) {
    const ahu = graph.blocks!.find(block => block.name === `ahu_${index}`)!
    ahu.blocks!.push({ name: 'warmup_supply', block_type: 'Constant', inputs: [],
      outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 18 }] })
  }
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36ObcSupplyAirResetMigration(saved))
  const next = migrateG36ObcSupplyAirReset(saved, reference)
  for (const index of [1, 2]) {
    const blocks = next.graph.blocks!.find(block => block.name === `ahu_${index}`)!.blocks!
    assert.equal(blocks.filter(block => block.name === 'sat_weather_line').length, 1)
    assert.ok(!blocks.some(block => block.name === 'warmup_supply'))
  }
})
