import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AhuCoilValveFeedback, needsG36AhuCoilValveFeedbackMigration } from '../src/lib/g36AhuCoilValveFeedbackMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved and renamed G36 AHUs gain valve-position CDL tracking without replacing command edits', () => {
  const old = structuredClone(reference)
  old.name = 'RenamedG36Building'
  old.inputs = old.inputs!.filter(port => !/_actual_(heating|cooling)_coil_valve_position$/.test(port.name))
  old.outputs = old.outputs!.filter(port => !/_(heating|cooling)_coil_valve_tracking_alarm_level$/.test(port.name))
  old.brick_model!.entities = old.brick_model!.entities.filter(entity =>
    !/_actual_(heating|cooling)_coil_valve_position$/.test(entity.id))
  old.brick_model!.relationships = old.brick_model!.relationships.filter(relation =>
    !/_actual_(heating|cooling)_coil_valve_position$/.test(relation.object))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !/_actual_(heating|cooling)_coil_valve_position$/.test(from) &&
      !/\.actual_(heating|cooling)_coil_valve_position$/.test(to) &&
      !/_(heating|cooling)_coil_valve_tracking_alarm_level$/.test(to)
  })
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.inputs = ahu.inputs!.filter(port => !/^actual_(heating|cooling)_coil_valve_position$/.test(port.name))
    ahu.outputs = ahu.outputs!.filter(port => !/^(heating|cooling)_coil_valve_tracking_alarm_level$/.test(port.name))
    ahu.blocks = ahu.blocks!.filter(block => !/^(heating|cooling)_coil_valve_tracking_/.test(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !/^(heating|cooling)_coil_valve_tracking_/.test(from) &&
        !/^(heating|cooling)_coil_valve_tracking_/.test(to)
    })
  }
  const ahu = old.blocks!.find(block => block.name === 'ahu_1')!
  ahu.blocks!.push({ name: 'edited_valve', block_type: 'Constant', category: 'elementary',
    outputs: [{ name: 'y', type: 'Real', unit: 'fraction (0–1)' }],
    parameters: [{ name: 'k', type: 'Real', value: 0.4, unit: 'fraction (0–1)' }] })
  const commandEdge = ahu.connections!.find(edge => edgeEndpoints(edge)[1] === 'heating_coil_valve_position')!
  commandEdge.from = 'edited_valve.y'
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 21, y: 34 } } }
  assert.ok(needsG36AhuCoilValveFeedbackMigration(saved))
  const next = migrateG36AhuCoilValveFeedback(saved, reference)
  assert.equal(next.graph.inputs!.filter(port => /_actual_(heating|cooling)_coil_valve_position$/.test(port.name)).length, 4)
  assert.equal(next.graph.outputs!.filter(port => /_(heating|cooling)_coil_valve_tracking_alarm_level$/.test(port.name)).length, 4)
  assert.equal(next.graph.brick_model!.entities.filter(entity =>
    /_actual_(heating|cooling)_coil_valve_position$/.test(entity.id)).length, 4)
  assert.deepEqual(next.layout.ahu_1, { x: 21, y: 34 })
  const migratedAhu = next.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.equal(migratedAhu.blocks!.find(block => block.name === 'edited_valve')!.parameters![0].value, 0.4)
  assert.ok(migratedAhu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'edited_valve.y→heating_coil_valve_tracking_command_minus_actual.u1'))
  assert.equal(needsG36AhuCoilValveFeedbackMigration(next), false)
  assert.equal(migrateG36AhuCoilValveFeedback(next, reference), next)
})
