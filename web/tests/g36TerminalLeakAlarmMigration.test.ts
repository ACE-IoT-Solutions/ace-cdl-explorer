import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalLeakAlarms, migrateG36TerminalLeakTiming, migrateG36TerminalLeakFlowHysteresis, needsG36TerminalLeakAlarmMigration, needsG36TerminalLeakTimingMigration, needsG36TerminalLeakFlowHysteresisMigration } from '../src/lib/g36TerminalLeakAlarmMigration'
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

test('legacy leakage thresholds adopt OBC defaults without replacing edited limits', () => {
  const graph = structuredClone(reference)
  graph.name = 'My commissioned G36 controls'
  for (const zone of graph.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    for (const [constant, comparator] of [
      ['leak_damper_closed_limit', 'leak_damper_command_closed'],
      ['leak_valve_closed_limit', 'leak_valve_position_closed'],
    ]) {
      zone.blocks!.find(block => block.name === constant)!.parameters![0].value = zone.name === 'zone_02' ? 0.1 : 0.005
      zone.blocks!.find(block => block.name === comparator)!.parameters = []
    }
  }
  graph.blocks!.find(block => block.name === 'zone_02')!.blocks!
    .find(block => block.name === 'leak_valve_position_closed')!.notes = 'Keep my comparator note'
  const preserved = graph.blocks!.find(block => block.name === 'zone_03')!.blocks!
    .find(block => block.name === 'leak_valve_position_closed')!
  preserved.parameters = [{ name: 'h', value: 0.01 }]
  const saved: Project = { version: 1, name: graph.name, graph, scenario: structuredClone(defaultScenario), layout: {} }
  assert.ok(needsG36TerminalLeakAlarmMigration(saved))
  const next = migrateG36TerminalLeakAlarms(saved, reference)
  for (const [zoneName, value] of [['zone_01', 0.05], ['zone_02', 0.1]] as const) {
    const zone = next.graph.blocks!.find(block => block.name === zoneName)!
    for (const [constant, comparator] of [
      ['leak_damper_closed_limit', 'leak_damper_command_closed'],
      ['leak_valve_closed_limit', 'leak_valve_position_closed'],
    ]) {
      assert.equal(zone.blocks!.find(block => block.name === constant)!.parameters![0].value, value)
      assert.equal(zone.blocks!.find(block => block.name === comparator)!.parameters![0].value, value / 2)
    }
  }
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_02')!.blocks!
    .find(block => block.name === 'leak_valve_position_closed')!.notes, 'Keep my comparator note')
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_03')!.blocks!
    .find(block => block.name === 'leak_valve_closed_limit')!.parameters![0].value, 0.005)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_03')!.blocks!
    .find(block => block.name === 'leak_valve_position_closed')!.parameters![0].value, 0.01)
  assert.equal(needsG36TerminalLeakAlarmMigration(next), false)
})

test('saved leaking-valve alarms time the complete condition and keep edited delays', () => {
  const old=structuredClone(reference)
  old.name='Commissioned G36 sequence'
  for(const zone of old.blocks!.filter(block=>block.block_type==='VAVBoxController')) {
    zone.connections=zone.connections!.filter(edge=>!['leak_valve_closed_delay.u',
      'leak_valve_fan_and_closed.u1','leak_valve_select.u2'].includes(edgeEndpoints(edge)[1]))
    zone.connections.push(
      {from:'leak_valve_position_closed.y',to:'leak_valve_closed_delay.u'},
      {from:'leak_valve_closed_delay.y',to:'leak_valve_fan_and_closed.u1'},
      {from:'leak_valve_condition.y',to:'leak_valve_select.u2'},
    )
    zone.blocks!.find(block=>block.name==='leak_valve_temperature_high')!.parameters=[]
  }
  const first=old.blocks!.find(block=>block.name==='zone_01')!
  first.blocks!.find(block=>block.name==='leak_valve_closed_delay')!.parameters![0].value=1200
  const second=old.blocks!.find(block=>block.name==='zone_02')!
  second.blocks!.find(block=>block.name==='leak_valve_temperature_high')!.parameters=[{name:'h',type:'Real',value:0.5}]
  const saved:Project={version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{zone_01:{x:17,y:23}}}
  assert.ok(needsG36TerminalLeakTimingMigration(saved))
  const next=migrateG36TerminalLeakTiming(saved)
  assert.deepEqual(next.layout.zone_01,{x:17,y:23})
  for(const zone of next.graph.blocks!.filter(block=>block.block_type==='VAVBoxController')) {
    for(const [target,source] of [
      ['leak_valve_fan_and_closed.u1','leak_valve_position_closed.y'],
      ['leak_valve_closed_delay.u','leak_valve_condition.y'],
      ['leak_valve_select.u2','leak_valve_closed_delay.y'],
    ]) assert.deepEqual(zone.connections!.filter(edge=>edgeEndpoints(edge)[1]===target)
      .map(edge=>edgeEndpoints(edge)[0]),[source])
  }
  assert.equal(next.graph.blocks!.find(block=>block.name==='zone_01')!.blocks!
    .find(block=>block.name==='leak_valve_closed_delay')!.parameters![0].value,1200)
  assert.equal(next.graph.blocks!.find(block=>block.name==='zone_01')!.blocks!
    .find(block=>block.name==='leak_valve_temperature_high')!.parameters?.find(p=>p.name==='h')?.value,0.25)
  assert.equal(next.graph.blocks!.find(block=>block.name==='zone_02')!.blocks!
    .find(block=>block.name==='leak_valve_temperature_high')!.parameters?.find(p=>p.name==='h')?.value,0.5)
  assert.equal(needsG36TerminalLeakTimingMigration(next),false)
  assert.equal(migrateG36TerminalLeakTiming(next),next)
})

test('saved leaking-damper comparators gain OBC flow hysteresis without replacing edits', () => {
  const old=structuredClone(reference)
  old.name='Commissioned G36 sequence'
  for(const zone of old.blocks!.filter(block=>block.block_type==='VAVBoxController'))
    zone.blocks!.find(block=>block.name==='leak_damper_flow_high')!.parameters=[]
  old.blocks!.find(block=>block.name==='zone_02')!.blocks!
    .find(block=>block.name==='leak_damper_flow_high')!.parameters=[{name:'h',type:'Real',value:0.025}]
  const saved:Project={version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{zone_01:{x:17,y:23}}}
  assert.ok(needsG36TerminalLeakFlowHysteresisMigration(saved))
  const next=migrateG36TerminalLeakFlowHysteresis(saved)
  assert.deepEqual(next.layout.zone_01,{x:17,y:23})
  for(const zone of next.graph.blocks!.filter(block=>block.block_type==='VAVBoxController'))
    assert.equal(zone.blocks!.find(block=>block.name==='leak_damper_flow_high')!.parameters!
      .find(parameter=>parameter.name==='h')!.value,zone.name==='zone_02'?0.025:0.05)
  assert.equal(needsG36TerminalLeakFlowHysteresisMigration(next),false)
  assert.equal(migrateG36TerminalLeakFlowHysteresis(next),next)
})
