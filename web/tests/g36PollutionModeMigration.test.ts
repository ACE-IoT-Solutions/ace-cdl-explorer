import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PollutionMode } from '../src/lib/g36PollutionModeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 graph gains CDL pollution mode without replacing tuned AHU controls', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks?.filter(block => block.name !== 'outdoor_air_pollution_mode')
  old.inputs = old.inputs?.filter(port => !port.name.startsWith('pollution_'))
  old.outputs = old.outputs?.filter(port => !port.name.startsWith('pollution_'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end.startsWith('pollution_') || end.startsWith('outdoor_air_pollution_mode.')))
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.inputs = ahu.inputs?.filter(port => port.name !== 'pollution_active')
    ahu.blocks = ahu.blocks?.filter(block => !['pollution_clear', 'economizer_pollution_gate'].includes(block.name))
    ahu.connections = ahu.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
      end.startsWith('pollution_') || end.startsWith('economizer_pollution_gate.') || end.startsWith('pollution_clear.')))
    ahu.connections?.push({from:'economizer_temperature_ok.y',to:'economizer_enabled.u2'})
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'economizer_mixed_air_pi')!.parameters![0].value = 0.23
  const saved: Project = {version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:19,y:31}}}
  const migrated = migrateG36PollutionMode(saved, reference)
  assert.ok(migrated.graph.blocks?.some(block => block.name === 'outdoor_air_pollution_mode'))
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks
    ?.find(block => block.name === 'economizer_mixed_air_pi')?.parameters?.[0].value, 0.23)
  for (const ahu of migrated.graph.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    assert.deepEqual(ahu.connections?.filter(edge => edgeEndpoints(edge)[1] === 'economizer_enabled.u2')
      .map(edge => edgeEndpoints(edge)[0]), ['economizer_pollution_gate.y'])
  }
  assert.deepEqual(migrated.layout.ahu_1,{x:19,y:31})
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

test('saved first-slice pollution mode gains operator duration and daily reminder without losing limits', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs?.filter(port => port.name !== 'pollution_on_duration_s')
  old.outputs = old.outputs?.filter(port => port.name !== 'pollution_level_three_reminder')
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end === 'pollution_on_duration_s' || end === 'pollution_level_three_reminder' ||
    end.endsWith('.on_duration_s') || end.endsWith('.level_three_reminder')))
  const mode = old.blocks?.find(block => block.name === 'outdoor_air_pollution_mode')!
  mode.inputs = mode.inputs?.filter(port => port.name !== 'on_duration_s')
  mode.outputs = mode.outputs?.filter(port => port.name !== 'level_three_reminder')
  mode.blocks = mode.blocks?.filter(block => !['on_elapsed','on_not_expired','daily_reminder_clock',
    'reminder_active','reminder_level'].includes(block.name))
  mode.blocks?.push({name:'on_expired',block_type:'Buildings.Controls.OBC.CDL.Logical.TrueDelay',
    inputs:[{name:'u',type:'Boolean'}],outputs:[{name:'y',type:'Boolean'}],parameters:[{name:'delayTime',value:3600}]})
  mode.blocks?.push({name:'on_not_expired',block_type:'Buildings.Controls.OBC.CDL.Logical.Not',
    inputs:[{name:'u',type:'Boolean'}],outputs:[{name:'y',type:'Boolean'}]})
  mode.connections = mode.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    ['on_elapsed.','on_not_expired.','daily_reminder_clock.','reminder_active.','reminder_level.'].some(prefix => end.startsWith(prefix)) ||
    end === 'on_duration_s' || end === 'level_three_reminder'))
  mode.connections?.push({from:'is_on.y',to:'on_expired.u'},{from:'on_expired.y',to:'on_not_expired.u'},
    {from:'on_not_expired.y',to:'on_active.u2'})
  mode.blocks!.find(block => block.name === 'pm25_limit')!.parameters![0].value = 75
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated=migrateG36PollutionMode(saved,reference)
  const updated=migrated.graph.blocks?.find(block => block.name === 'outdoor_air_pollution_mode')!
  assert.equal(updated.blocks?.find(block => block.name === 'pm25_limit')?.parameters?.[0].value,75)
  assert.ok(updated.blocks?.some(block => block.name === 'on_elapsed'))
  assert.ok(updated.blocks?.some(block => block.name === 'daily_reminder_clock'))
  assert.equal(updated.connections?.filter(edge => edgeEndpoints(edge)[1] === 'on_active.u2').length,1)
  assert.ok(migrated.graph.inputs?.some(port => port.name === 'pollution_on_duration_s'))
  assert.ok(migrated.graph.outputs?.some(port => port.name === 'pollution_level_three_reminder'))
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})
