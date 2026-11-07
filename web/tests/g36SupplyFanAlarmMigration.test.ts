import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36SupplyFanAlarms, needsG36SupplyFanAlarmMigration } from '../src/lib/g36SupplyFanAlarmMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 AHUs gain editable fan status alarms while retaining tuned controls', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_supply_fan_alarm_level$/.test(port.name))
  old.connections = old.connections!.filter(edge =>
    !/^ahu_[12]_supply_fan_alarm_level$/.test(edgeEndpoints(edge)[1]))
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !block.name.startsWith('fan_alarm_'))
    ahu.outputs = ahu.outputs!.filter(port => port.name !== 'supply_fan_alarm_level')
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('fan_alarm_') && !to.startsWith('fan_alarm_') &&
        to !== 'supply_fan_alarm_level'
    })
  }
  old.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'fan_gain')!.parameters![0].value = 0.1
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 30, y: 50 } } }
  assert.ok(needsG36SupplyFanAlarmMigration(saved))
  const next = migrateG36SupplyFanAlarms(saved, reference)
  assert.equal(next.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'fan_gain')!.parameters![0].value, 0.1)
  assert.deepEqual(next.layout.ahu_1, { x: 30, y: 50 })
  for (const index of [1, 2]) {
    const ahu = next.graph.blocks!.find(block => block.name === `ahu_${index}`)!
    assert.ok(ahu.blocks!.some(block => block.name === 'fan_alarm_priority'))
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'supply_fan_alarm_level').length, 1)
    assert.equal(next.graph.inputs!.find(port => port.name === `ahu_${index}_supply_fan_proven`)!.brick_class,
      'Fan_Status')
  }
  assert.equal(needsG36SupplyFanAlarmMigration(next), false)
  assert.equal(migrateG36SupplyFanAlarms(next, reference), next)
})
