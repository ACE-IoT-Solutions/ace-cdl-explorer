import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FanSpeedFeedback, needsG36FanSpeedFeedbackMigration } from '../src/lib/g36FanSpeedFeedbackMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain measured speed and editable tracking alarms', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => !/_actual_fan_speed$/.test(port.name))
  old.outputs = old.outputs!.filter(port => !/_fan_speed_tracking_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !/_actual_fan_speed$/.test(from) && !/\.actual_fan_speed$/.test(to) &&
      !/_fan_speed_tracking_alarm_level$/.test(to)
  })
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.inputs = ahu.inputs!.filter(port => port.name !== 'actual_fan_speed')
    ahu.outputs = ahu.outputs!.filter(port => port.name !== 'fan_speed_tracking_alarm_level')
    ahu.blocks = ahu.blocks!.filter(block => !block.name.startsWith('fan_speed_tracking_'))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('fan_speed_tracking_') && !to.startsWith('fan_speed_tracking_')
    })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'fan_gain')!.parameters![0].value = 0.42
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 19, y: 31 } } }
  assert.ok(needsG36FanSpeedFeedbackMigration(saved))
  const next = migrateG36FanSpeedFeedback(saved, reference)
  assert.equal(next.graph.inputs!.filter(port => /_actual_fan_speed$/.test(port.name)).length, 2)
  assert.equal(next.graph.outputs!.filter(port => /_fan_speed_tracking_alarm_level$/.test(port.name)).length, 2)
  assert.deepEqual(next.layout.ahu_1, { x: 19, y: 31 })
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'fan_gain')!.parameters![0].value, 0.42)
  assert.equal(needsG36FanSpeedFeedbackMigration(next), false)
  assert.equal(migrateG36FanSpeedFeedback(next, reference), next)
})
