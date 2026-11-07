import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36AirflowSensorAlarms, needsG36AirflowSensorAlarmMigration } from '../src/lib/g36AirflowSensorAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 VAVs gain independent sensor alarms without replacing control edits', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs!.filter(port => !/_airflow_sensor_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge => !/_airflow_sensor_alarm_level$/.test(edgeEndpoints(edge)[1]))
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block => !block.name.startsWith('sensor_'))
    zone.outputs = zone.outputs!.filter(port => port.name !== 'airflow_sensor_alarm_level')
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('sensor_') && !to.startsWith('sensor_') && to !== 'airflow_sensor_alarm_level'
    })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36AirflowSensorAlarmMigration(saved))
  const next = migrateG36AirflowSensorAlarms(saved, reference)
  const zone = next.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  assert.equal(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === 'sensor_alarm_select.u2').length, 1)
  assert.equal(next.graph.outputs!.filter(port => port.name.endsWith('_airflow_sensor_alarm_level')).length, 10)
  assert.equal(needsG36AirflowSensorAlarmMigration(next), false)
  assert.equal(migrateG36AirflowSensorAlarms(next, reference), next)
})
