import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36HighDischargeAlarm, needsG36HighDischargeAlarmMigration } from '../src/lib/g36HighDischargeAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 high-discharge alarms gain CDL fan and occupied gating without losing tuning', () => {
  const old = structuredClone(reference)
  old.name = 'EditedG36Building'
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block => !['high_discharge_eligible', 'high_discharge_condition', 'high_discharge_alarm_priority'].includes(block.name))
    zone.outputs = zone.outputs!.filter(port => port.name !== 'high_discharge_alarm_level')
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('high_discharge_eligible.') && !from.startsWith('high_discharge_condition.') &&
        !from.startsWith('high_discharge_alarm_priority.') && !to.startsWith('high_discharge_eligible.') &&
        !to.startsWith('high_discharge_condition.') && !to.startsWith('high_discharge_alarm_priority.') &&
        to !== 'high_discharge_alarm_level'
    })
    zone.connections.push({from:'high_discharge_detected.y',to:'high_discharge_delay.u'})
  }
  old.outputs = old.outputs!.filter(port => !port.name.endsWith('_high_discharge_alarm_level'))
  old.connections = old.connections!.filter(edge => !edge.to?.endsWith('_high_discharge_alarm_level'))
  const zoneOne = old.blocks!.find(block => block.name === 'zone_01')!
  zoneOne.blocks!.find(block => block.name === 'high_discharge_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value = 420
  const saved: Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{zone_01:{x:17,y:23}}}
  assert.ok(needsG36HighDischargeAlarmMigration(saved))
  const next = migrateG36HighDischargeAlarm(saved, reference)
  assert.deepEqual(next.layout.zone_01, {x:17,y:23})
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'high_discharge_delay')!.parameters!
    .find(parameter => parameter.name === 'delayTime')!.value, 420)
  for (const zone of next.graph.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    assert.ok(zone.blocks!.some(block => block.name === 'high_discharge_condition'))
    assert.ok(zone.blocks!.some(block => block.name === 'high_discharge_alarm_priority'))
    assert.ok(zone.outputs!.some(port => port.name === 'high_discharge_alarm_level'))
    assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'high_discharge_delay.u')
      .map(edge => edgeEndpoints(edge)[0]), ['high_discharge_condition.y'])
  }
  assert.equal(next.graph.outputs!.filter(port => port.name.endsWith('_high_discharge_alarm_level')).length, 10)
  assert.equal(needsG36HighDischargeAlarmMigration(next), false)
  assert.equal(migrateG36HighDischargeAlarm(next, reference), next)
})

test('partially saved high-discharge level repairs a missing graph output wire', () => {
  const graph = structuredClone(reference)
  graph.connections = graph.connections!.filter(edge => edgeEndpoints(edge).join('→') !==
    'zone_01.high_discharge_alarm_level→zone_01_high_discharge_alarm_level')
  const saved: Project = {version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{}}
  assert.equal(needsG36HighDischargeAlarmMigration(saved), true)
  const repaired = migrateG36HighDischargeAlarm(saved, reference)
  assert.equal(repaired.graph.connections!.filter(edge => edgeEndpoints(edge).join('→') ===
    'zone_01.high_discharge_alarm_level→zone_01_high_discharge_alarm_level').length, 1)
  assert.equal(needsG36HighDischargeAlarmMigration(repaired), false)
})
