import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ZoneModes } from '../src/lib/g36ZoneModesMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const modeBlock = /^(mode_|warmup_error$|cooldown_error$|setback_error$|setup_error$|warmup_needed$|cooldown_needed$|setback_needed$|setup_needed$|warmup_mode$|cooldown_mode$|preconditioning_needed$|occupied_setpoint_requested$)/

test('saved G36 zones gain local CDL modes and retain tuned setpoints', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs!.filter(port => !port.name.endsWith('_conditioning_state'))
  old.connections = old.connections!.filter(edge => !edgeEndpoints(edge)[1].endsWith('_conditioning_state'))
  for (const zone of old.blocks!.filter(block => block.block_type === 'VAVBoxController')) {
    zone.outputs = zone.outputs!.filter(port => port.name !== 'conditioning_state')
    zone.blocks = zone.blocks!.filter(block => !modeBlock.test(block.name))
    zone.connections = zone.connections!.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return !modeBlock.test(from.split('.')[0]) && !modeBlock.test(to.split('.')[0]) && to !== 'conditioning_state'
    })
    zone.connections.push({from:'preconditioning_mode',to:'conditioning_mode.u2'})
    zone.connections.push({from:'preconditioning_mode',to:'heating_warmup_airflow.u3'})
    zone.connections.push({from:'occupancy_latch.y',to:'heat_setpoint_standby.u3'})
    zone.connections.push({from:'occupancy_latch.y',to:'cool_setpoint_standby.u3'})
  }
  const first = old.blocks!.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{zone_01:{x:42,y:24}}}
  const next = migrateG36ZoneModes(saved,reference)
  const migrated = next.graph.blocks!.find(block => block.name === 'zone_01')!
  assert.equal(migrated.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value,22)
  assert.ok(migrated.blocks!.some(block => block.name === 'mode_select_schedule'))
  assert.equal(migrated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'conditioning_mode.u2').length,1)
  assert.equal(migrated.connections!.filter(edge => edgeEndpoints(edge)[1] === 'heating_warmup_airflow.u3').length,1)
  assert.equal(next.graph.outputs!.filter(port => port.name.endsWith('_conditioning_state')).length,10)
  assert.deepEqual(next.layout.zone_01,{x:42,y:24})
  assert.equal(migrateG36ZoneModes(next,reference),next)
})
