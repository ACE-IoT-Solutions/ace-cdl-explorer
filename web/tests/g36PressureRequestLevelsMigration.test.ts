import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PressureRequestLevels, needsG36PressureRequestLevelsMigration } from '../src/lib/g36PressureRequestLevelsMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const former = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block

test('saved binary pressure requests migrate to editable 0–3 levels without replacing zone tuning', () => {
  const old = structuredClone(former)
  const first = old.blocks!.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'cool_setpoint')!.parameters![0].value = 25.5
  const project: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36PressureRequestLevelsMigration(project))
  const next = migrateG36PressureRequestLevels(project, reference)
  assert.equal(next.layout.zone_01.x, 17)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'cool_setpoint')!.parameters![0].value, 25.5)
  for (const zone of next.graph.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    assert.ok(zone.blocks!.some(block => block.name === 'pressure_importance_select'))
    assert.ok(zone.blocks!.some(block => block.name === 'low_flow_importance_gate'))
    assert.ok(!zone.blocks!.some(block => block.name === 'pressure_request_integer'))
    for (const target of ['pressure_reset_request', 'low_flow_eligible.u1', 'low_flow_occupied_gate.u1']) {
      assert.equal(zone.connections!.filter(edge => edgeEndpoints(edge)[1] === target).length, 1, `${zone.name}.${target}`)
    }
    assert.equal(zone.connections!.find(edge => edgeEndpoints(edge)[1] === 'pressure_reset_request')!.from,
      'pressure_importance_select.y')
  }
  assert.equal(needsG36PressureRequestLevelsMigration(next), false)
  assert.equal(migrateG36PressureRequestLevels(next, reference), next)
})

test('a saved pressure tier branch gains synchronous sampling without losing tuning', () => {
  const old = structuredClone(reference)
  const replaced = new Set(['pressure_damper_sample','pressure_setpoint_sample','pressure_measured_sample',
    'pressure_damper_delay','pressure_damper_flow_eligible'])
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.blocks = zone.blocks!.filter(block => !replaced.has(block.name))
    zone.blocks.push({name:'pressure_damper_high',block_type:'Greater',category:'elementary',
      inputs:[{name:'u1',type:'Real'},{name:'u2',type:'Real'}],outputs:[{name:'y',type:'Boolean'}]})
    zone.connections = zone.connections!.filter(edge => {
      const [from,to]=edgeEndpoints(edge)
      return !replaced.has(from.split('.')[0])&&!replaced.has(to.split('.')[0])
    })
  }
  const first=old.blocks!.find(block=>block.name==='zone_01')!
  first.blocks!.find(block=>block.name==='pressure_importance')!.parameters![0].value=0
  first.blocks!.find(block=>block.name==='pressure_damper_hysteresis')!.parameters![0].value=0.8
  const saved:Project={version:1,name:old.name,graph:old,
    scenario:structuredClone(defaultScenario),layout:{zone_01:{x:17,y:23}}}
  assert.ok(needsG36PressureRequestLevelsMigration(saved))
  const next=migrateG36PressureRequestLevels(saved,reference)
  const zone=next.graph.blocks!.find(block=>block.name==='zone_01')!
  assert.equal(zone.blocks!.find(block=>block.name==='pressure_importance')!.parameters![0].value,0)
  assert.equal(zone.blocks!.find(block=>block.name==='pressure_damper_hysteresis')!.parameters![0].value,0.8)
  assert.ok(!zone.blocks!.some(block=>block.name==='pressure_damper_high'))
  for(const target of ['pressure_severe_eligible.u1','pressure_moderate_eligible.u1'])
    assert.deepEqual(zone.connections!.filter(edge=>edgeEndpoints(edge)[1]===target)
      .map(edge=>edgeEndpoints(edge)[0]),['pressure_damper_flow_eligible.y'])
  assert.deepEqual(next.layout.zone_01,{x:17,y:23})
  assert.equal(needsG36PressureRequestLevelsMigration(next),false)
})
