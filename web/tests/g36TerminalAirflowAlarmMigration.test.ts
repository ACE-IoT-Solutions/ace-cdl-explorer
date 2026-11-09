import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalAirflowAlarms, needsG36TerminalAirflowAlarmMigration } from '../src/lib/g36TerminalAirflowAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 VAVs gain tiered measured-airflow alarms without losing control tuning', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs!.filter(port => !/_low_airflow_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge => !/_low_airflow_alarm_level$/.test(edgeEndpoints(edge)[1]))
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block => !block.name.startsWith('low_flow_seventy_') &&
      !['low_flow_eligible', 'low_flow_occupied_gate', 'low_flow_alarm_zero', 'low_flow_alarm_three',
        'low_flow_alarm_two', 'low_flow_alarm_moderate', 'low_flow_alarm_priority'].includes(block.name))
    zone.outputs = zone.outputs!.filter(port => port.name !== 'low_airflow_alarm_level')
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('low_flow_seventy_') && !to.startsWith('low_flow_seventy_') &&
        !['low_flow_eligible', 'low_flow_occupied_gate', 'low_flow_alarm_zero', 'low_flow_alarm_three',
          'low_flow_alarm_two', 'low_flow_alarm_moderate', 'low_flow_alarm_priority'].some(name =>
          from.startsWith(`${name}.`) || to.startsWith(`${name}.`)) && to !== 'low_airflow_alarm_level'
    })
    zone.connections!.push({ from: 'low_flow_condition.y', to: 'low_flow_delay.u' })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36TerminalAirflowAlarmMigration(saved))
  const next = migrateG36TerminalAirflowAlarms(saved, reference)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  for (let index = 1; index <= 10; index++) {
    const zone = next.graph.blocks!.find(block => block.name === `zone_${String(index).padStart(2, '0')}`)!
    assert.ok(zone.blocks!.some(block => block.name === 'low_flow_alarm_priority'))
    assert.equal(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'low_flow_delay.u').length, 1)
    assert.equal(zone.connections!.find(edge => edgeEndpoints(edge)[1] === 'low_flow_delay.u')!.from,
      'low_flow_severe_fan_condition.y')
    assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'low_flow_alarm_priority.u2')
      .map(edge => edgeEndpoints(edge)[0]), ['low_flow_occupied_gate.y'])
  }
  assert.equal(needsG36TerminalAirflowAlarmMigration(next), false)
  assert.equal(migrateG36TerminalAirflowAlarms(next, reference), next)
})

test('saved tiered alarms gain fan proof suppression without replacing edited thresholds', () => {
  const old = structuredClone(reference)
  old.connections = old.connections!.filter(edge =>
    !/zone_\d{2}\.supply_fan_proven$/.test(edgeEndpoints(edge)[1]))
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.inputs = zone.inputs!.filter(port => port.name !== 'supply_fan_proven')
    zone.blocks = zone.blocks!.filter(block => !['low_flow_fan_ready', 'low_flow_occupied_and_fan'].includes(block.name))
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from !== 'supply_fan_proven' && !from.startsWith('low_flow_fan_ready.') &&
        !from.startsWith('low_flow_occupied_and_fan.') &&
        !to.startsWith('low_flow_fan_ready.') && !to.startsWith('low_flow_occupied_and_fan.')
    })
    zone.connections.push({ from: 'local_occupied.y', to: 'low_flow_eligible.u1' },
      { from: 'local_occupied.y', to: 'low_flow_occupied_gate.u1' })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'low_flow_seventy_setpoint')!.parameters![0].value = 0.65
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36TerminalAirflowAlarmMigration(saved))
  const next = migrateG36TerminalAirflowAlarms(saved, reference)
  const zone = next.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks!.find(block => block.name === 'low_flow_seventy_setpoint')!.parameters![0].value, 0.65)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'low_flow_eligible.u1')
    .map(edge => edgeEndpoints(edge)[0]), ['low_flow_importance_gate.y'])
  assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'low_flow_importance_gate.u1')
    .map(edge => edgeEndpoints(edge)[0]), ['low_flow_occupied_and_fan.y'])
  assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'zone_01.supply_fan_proven').length, 1)
  assert.equal(needsG36TerminalAirflowAlarmMigration(next), false)
})

test('released low-flow timing upgrades while retaining a tuned fan qualification delay', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block =>
      !['low_flow_severe_fan_condition', 'low_flow_seventy_output_gate'].includes(block.name))
    const fanReady = zone.blocks.find(block => block.name === 'low_flow_fan_ready')!
    fanReady.parameters!.find(parameter => parameter.name === 'delayTime')!.value = 1200
    fanReady.parameters!.find(parameter => parameter.name === 'delayOnInit')!.value = true
  }
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36TerminalAirflowAlarmMigration(saved))
  const next = migrateG36TerminalAirflowAlarms(saved, reference)
  for (const zone of next.graph.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    const fanReady = zone.blocks!.find(block => block.name === 'low_flow_fan_ready')!
    assert.equal(fanReady.parameters!.find(parameter => parameter.name === 'delayTime')!.value, 1200)
    assert.equal(fanReady.parameters!.find(parameter => parameter.name === 'delayOnInit')!.value, false)
    for (const [target, source] of [
      ['low_flow_delay.u', 'low_flow_severe_fan_condition.y'],
      ['low_flow_seventy_output_gate.u1', 'low_flow_seventy_delay.y'],
      ['low_flow_alarm_priority.u2', 'low_flow_occupied_gate.y'],
      ['low_airflow_alarm', 'low_flow_occupied_gate.y'],
    ]) {
      assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
        .map(edge => edgeEndpoints(edge)[0]), [source])
    }
  }
  assert.equal(needsG36TerminalAirflowAlarmMigration(next), false)
})
