import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36LowDischargeAlarms, needsG36LowDischargeAlarmMigration } from '../src/lib/g36LowDischargeAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 zones gain measured low-discharge alarms and retain edited setpoints', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => port.name !== 'hot_water_plant_proven')
  old.outputs = old.outputs!.filter(port => !port.name.endsWith('_low_discharge_temperature_alarm_level'))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return from !== 'hot_water_plant_proven' && !to.endsWith('.hot_water_plant_proven') &&
      !to.endsWith('_low_discharge_temperature_alarm_level')
  })
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.inputs = zone.inputs!.filter(port => port.name !== 'hot_water_plant_proven')
    zone.outputs = zone.outputs!.filter(port => port.name !== 'low_discharge_temperature_alarm_level')
    zone.blocks = zone.blocks!.filter(block => !block.name.startsWith('low_discharge_'))
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('low_discharge_') && !to.startsWith('low_discharge_') &&
        to !== 'low_discharge_temperature_alarm_level'
    })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36LowDischargeAlarmMigration(saved))
  const next = migrateG36LowDischargeAlarms(saved, reference)
  const zone = next.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.equal(next.graph.inputs!.filter(port => port.name === 'hot_water_plant_proven').length, 1)
  assert.equal(next.graph.outputs!.filter(port => port.name.endsWith('_low_discharge_temperature_alarm_level')).length, 10)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}.hot_water_plant_proven`).length, 1)
  }
  assert.equal(needsG36LowDischargeAlarmMigration(next), false)
  assert.equal(migrateG36LowDischargeAlarms(next, reference), next)
})

test('saved low-discharge alarms move proof gating after timers and keep edited delays', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block => block.name !== 'low_discharge_fan_and_plant')
    const rewritten = new Map([
      ['low_discharge_three_delay.u', 'low_discharge_three_condition.y'],
      ['low_discharge_two_delay.u', 'low_discharge_two_condition.y'],
      ['low_discharge_three_condition.u2', 'low_discharge_below_eight.y'],
      ['low_discharge_two_condition.u2', 'low_discharge_below_seventeen.y'],
      ['low_discharge_moderate.u2', 'low_discharge_three_delay.y'],
      ['low_discharge_priority.u2', 'low_discharge_two_delay.y'],
      ['low_discharge_occupied_and_plant.u2', 'hot_water_plant_proven'],
    ])
    zone.connections = zone.connections!.filter(edge =>
      !['low_discharge_fan_and_plant.u1', 'low_discharge_fan_and_plant.u2'].includes(edgeEndpoints(edge)[1]))
      .map(edge => ({...edge, from: rewritten.get(edgeEndpoints(edge)[1]) ?? edgeEndpoints(edge)[0]}))
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'low_discharge_three_delay')!.parameters![0].value = 420
  const saved: Project = {version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: {zone_01: {x: 17, y: 23}}}
  assert.ok(needsG36LowDischargeAlarmMigration(saved))
  const next = migrateG36LowDischargeAlarms(saved, reference)
  const zone = next.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks!.find(block => block.name === 'low_discharge_three_delay')!.parameters![0].value, 420)
  assert.deepEqual(next.layout.zone_01, {x: 17, y: 23})
  for (const [target, source] of [
    ['low_discharge_three_delay.u', 'low_discharge_below_eight.y'],
    ['low_discharge_moderate.u2', 'low_discharge_three_condition.y'],
    ['low_discharge_occupied_and_plant.u2', 'low_discharge_fan_and_plant.y'],
  ]) assert.deepEqual(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
    .map(edge => edgeEndpoints(edge)[0]), [source])
  assert.equal(needsG36LowDischargeAlarmMigration(next), false)
  assert.equal(migrateG36LowDischargeAlarms(next, reference), next)
})
