import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalLeakAlarms, needsG36TerminalLeakAlarmMigration } from '../src/lib/g36TerminalLeakAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 zones gain actuator sensor points and editable leakage alarms', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => !/_actual_(damper|reheat_valve)_position$/.test(port.name))
  old.outputs = old.outputs!.filter(port => !/_leaking_(damper|reheat_valve)_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !/_actual_reheat_valve_position$/.test(from) &&
      !/\.actual_reheat_valve_position$/.test(to) &&
      !/_leaking_(damper|reheat_valve)_alarm_level$/.test(to)
  })
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.inputs = zone.inputs!.filter(port => port.name !== 'actual_reheat_valve_position')
    zone.outputs = zone.outputs!.filter(port => !/^leaking_/.test(port.name))
    zone.blocks = zone.blocks!.filter(block => !block.name.startsWith('leak_'))
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('leak_') && !to.startsWith('leak_') && !to.startsWith('leaking_')
    })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36TerminalLeakAlarmMigration(saved))
  const next = migrateG36TerminalLeakAlarms(saved, reference)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.equal(next.graph.outputs!.filter(port => /_leaking_(damper|reheat_valve)_alarm_level$/.test(port.name)).length, 20)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks!.find(block => block.name === name)!
    assert.ok(zone.blocks!.some(block => block.name === 'leak_valve_select'))
    assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}.actual_reheat_valve_position`).length, 1)
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.equal(needsG36TerminalLeakAlarmMigration(next), false)
  assert.equal(migrateG36TerminalLeakAlarms(next, reference), next)
})
