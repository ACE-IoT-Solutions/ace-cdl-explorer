import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SatThresholds, needsG36SatThresholdMigration } from '../src/lib/g36SatThresholdMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved SAT request thresholds gain OBC hysteresis and startup timing without losing tuning', () => {
  const graph = structuredClone(reference)
  const zone = graph.blocks!.find(block => block.name === 'zone_01')!
  const untouched = structuredClone(graph.blocks!.find(block => block.name === 'zone_02')!)
  for (const [level, tuned] of [['two', 2.4], ['three', 3.2]] as const) {
    const name = `sat_${level}_error_high`
    const comparator = zone.blocks!.find(block => block.name === name)!
    comparator.block_type = 'Buildings.Controls.OBC.CDL.Reals.Greater'
    comparator.inputs = [{ name: 'u1', type: 'Real', unit: 'K' }, { name: 'u2', type: 'Real', unit: 'K' }]
    comparator.parameters = []
    comparator.notes = `Custom ${level} comparison note`
    zone.blocks!.push({ name: `sat_${level}_error_threshold`, block_type: 'Buildings.Controls.OBC.CDL.Reals.Sources.Constant',
      inputs: [], outputs: [{ name: 'y', type: 'Real', unit: 'K' }], parameters: [{ name: 'k', value: tuned }] })
    const input = zone.connections!.find(edge => edgeEndpoints(edge)[1] === `${name}.u`)!
    input.to = `${name}.u1`
    zone.connections!.push({ from: `sat_${level}_error_threshold.y`, to: `${name}.u2` })
    const delay = zone.blocks!.find(block => block.name === `sat_${level}_error_delay`)!
    delay.parameters!.find(parameter => parameter.name === 'delayOnInit')!.value = true
    delay.parameters!.find(parameter => parameter.name === 'delayTime')!.value = 150
  }
  const saved: Project = { version: 1, name: graph.name, graph, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 12, y: 34 } } }
  assert.ok(needsG36SatThresholdMigration(saved))
  const upgraded = migrateG36SatThresholds(saved, reference)
  assert.deepEqual(upgraded.layout, saved.layout)
  assert.deepEqual(upgraded.graph.blocks!.find(block => block.name === 'zone_02'), untouched)
  const updated = upgraded.graph.blocks!.find(block => block.name === 'zone_01')!
  for (const [level, tuned] of [['two', 2.4], ['three', 3.2]] as const) {
    const name = `sat_${level}_error_high`
    const comparator = updated.blocks!.find(block => block.name === name)!
    assert.ok(comparator.block_type.endsWith('Reals.GreaterThreshold'))
    assert.equal(comparator.parameters!.find(parameter => parameter.name === 't')!.value, tuned)
    assert.equal(comparator.parameters!.find(parameter => parameter.name === 'h')!.value, 0.25)
    assert.equal(comparator.notes, `Custom ${level} comparison note`)
    assert.ok(!updated.blocks!.some(block => block.name === `sat_${level}_error_threshold`))
    assert.deepEqual(updated.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}.u`).map(edge => edgeEndpoints(edge)[0]), ['sat_cooling_error.y'])
    const delay = updated.blocks!.find(block => block.name === `sat_${level}_error_delay`)!
    assert.equal(delay.parameters!.find(parameter => parameter.name === 'delayOnInit')!.value, false)
    assert.equal(delay.parameters!.find(parameter => parameter.name === 'delayTime')!.value, 150)
  }
  assert.equal(needsG36SatThresholdMigration(upgraded), false)
  assert.equal(migrateG36SatThresholds(upgraded, reference), upgraded)
})
