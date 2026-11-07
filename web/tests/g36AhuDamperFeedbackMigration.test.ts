import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AhuDamperFeedback, needsG36AhuDamperFeedbackMigration } from '../src/lib/g36AhuDamperFeedbackMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain physical damper points and CDL tracking alarms', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => !/_actual_(outdoor|return)_damper_position$/.test(port.name))
  old.outputs = old.outputs!.filter(port => !/_(outdoor|return)_damper_tracking_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !/_actual_(outdoor|return)_damper_position$/.test(from) &&
      !/\.actual_(outdoor|return)_damper_position$/.test(to) &&
      !/_(outdoor|return)_damper_tracking_alarm_level$/.test(to)
  })
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.inputs = ahu.inputs!.filter(port => !/^actual_(outdoor|return)_damper_position$/.test(port.name))
    ahu.outputs = ahu.outputs!.filter(port => !/^(outdoor|return)_damper_tracking_alarm_level$/.test(port.name))
    ahu.blocks = ahu.blocks!.filter(block => !/^(outdoor|return)_damper_tracking_/.test(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !/^(outdoor|return)_damper_tracking_/.test(from) &&
        !/^(outdoor|return)_damper_tracking_/.test(to)
    })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'outdoor_damper_physical_max')!.parameters![0].value = 0.6
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 21, y: 34 } } }
  assert.ok(needsG36AhuDamperFeedbackMigration(saved))
  const next = migrateG36AhuDamperFeedback(saved, reference)
  assert.equal(next.graph.inputs!.filter(port => /_actual_(outdoor|return)_damper_position$/.test(port.name)).length, 4)
  assert.equal(next.graph.outputs!.filter(port => /_(outdoor|return)_damper_tracking_alarm_level$/.test(port.name)).length, 4)
  assert.deepEqual(next.layout.ahu_1, { x: 21, y: 34 })
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'outdoor_damper_physical_max')!.parameters![0].value, 0.6)
  assert.equal(needsG36AhuDamperFeedbackMigration(next), false)
  assert.equal(migrateG36AhuDamperFeedback(next, reference), next)
})
