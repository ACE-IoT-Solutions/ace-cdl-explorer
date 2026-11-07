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
