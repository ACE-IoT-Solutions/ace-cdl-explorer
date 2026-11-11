import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PollutionMode } from '../src/lib/g36PollutionModeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved UTC daily reminder becomes an editable local-time CDL edge without losing its hour', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs?.filter(port => port.name !== 'local_day_seconds')
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end => end === 'local_day_seconds' || end.endsWith('.local_day_seconds')))
  for (const mode of [old, ...old.blocks!.filter(block => block.name.startsWith('outdoor_air_pollution_mode'))]) {
    const clockName = mode === old ? 'pollution_global_reminder_clock' : 'daily_reminder_clock'
    mode.inputs = mode.inputs?.filter(port => port.name !== 'local_day_seconds')
    mode.blocks = mode.blocks?.filter(block => !['_time', '_before', '_reached'].some(suffix => block.name === clockName + suffix))
    mode.connections = mode.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
      ['_time.', '_before.', '_reached.'].some(suffix => end.startsWith(clockName + suffix)) || end === clockName + '.u'))
    const clock = mode.blocks!.find(block => block.name === clockName)!
    clock.block_type = 'Buildings.Controls.OBC.CDL.Logical.Sources.SampleTrigger'
    clock.inputs = []
    clock.parameters = [{name:'period',value:86400},{name:'shift',value:28800}]
  }
  const saved: Project = {version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated = migrateG36PollutionMode(saved, reference)
  assert.ok(migrated.graph.inputs?.some(port => port.name === 'local_day_seconds'))
  for (const mode of [migrated.graph, ...migrated.graph.blocks!.filter(block => block.name.startsWith('outdoor_air_pollution_mode'))]) {
    const clockName = mode === migrated.graph ? 'pollution_global_reminder_clock' : 'daily_reminder_clock'
    assert.ok(mode.blocks?.find(block => block.name === clockName)?.block_type.endsWith('.Edge'))
    assert.equal(mode.blocks?.find(block => block.name === clockName + '_time')?.parameters?.[0].value, 28800)
    assert.equal(mode.connections?.filter(edge => edgeEndpoints(edge)[1] === clockName + '.u').length, 1)
  }
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

test('a custom non-daily saved reminder retains its original UTC period', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs?.filter(port => port.name !== 'local_day_seconds')
  const mode = old.blocks!.find(block => block.name === 'outdoor_air_pollution_mode')!
  mode.inputs = mode.inputs?.filter(port => port.name !== 'local_day_seconds')
  mode.blocks = mode.blocks?.filter(block => !['_time','_before','_reached'].some(suffix => block.name === 'daily_reminder_clock' + suffix))
  mode.connections = mode.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end === 'local_day_seconds' || end === 'daily_reminder_clock.u' ||
    ['_time.','_before.','_reached.'].some(suffix => end.startsWith('daily_reminder_clock' + suffix))))
  old.connections = old.connections?.filter(edge => edgeEndpoints(edge)[1] !== 'outdoor_air_pollution_mode.local_day_seconds')
  const clock = mode.blocks!.find(block => block.name === 'daily_reminder_clock')!
  clock.block_type = 'Buildings.Controls.OBC.CDL.Logical.Sources.SampleTrigger'
  clock.inputs = []
  clock.parameters = [{name:'period',value:43200},{name:'shift',value:3600}]
  const saved: Project = {version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated = migrateG36PollutionMode(saved,reference)
  assert.equal(migrated.graph.blocks!.find(block => block.name === 'outdoor_air_pollution_mode')!
    .blocks!.find(block => block.name === 'daily_reminder_clock')!.parameters?.[0].value,43200)
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

test('saved shared pollution mode becomes two editable AHU decisions without losing its limit', () => {
  const old=structuredClone(reference)
  old.blocks=old.blocks?.filter(block=>block.name!=='outdoor_air_pollution_mode_2')
  old.outputs=old.outputs?.filter(port=>!port.name.startsWith('ahu_2_pollution_'))
  old.connections=old.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>
    end.startsWith('outdoor_air_pollution_mode_2.')||end.startsWith('ahu_2_pollution_')))
  old.connections?.push({from:'outdoor_air_pollution_mode.active',to:'ahu_2.pollution_active'})
  for (const ahu of old.blocks?.filter(block=>block.block_type==='AHUController')??[]) {
    ahu.blocks=ahu.blocks?.filter(block=>block.name!=='minimum_oa_direct_base_select')
    ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>end.startsWith('minimum_oa_direct_base_select.')))
    ahu.connections?.push({from:'outdoor_air_select.y',to:'minimum_oa_command.u1'})
  }
  const shared=old.blocks!.find(block=>block.name==='outdoor_air_pollution_mode')!
  shared.blocks!.find(block=>block.name==='pm25_limit')!.parameters![0].value=75
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_2:{x:70,y:90}}}
  const migrated=migrateG36PollutionMode(saved,reference)
  const second=migrated.graph.blocks!.find(block=>block.name==='outdoor_air_pollution_mode_2')!
  assert.equal(second.blocks!.find(block=>block.name==='pm25_limit')!.parameters![0].value,75)
  assert.deepEqual(migrated.graph.connections?.filter(edge=>edgeEndpoints(edge)[1]==='ahu_2.pollution_active')
    .map(edge=>edgeEndpoints(edge)[0]),['outdoor_air_pollution_mode_2.active'])
  assert.deepEqual(migrated.graph.connections?.filter(edge=>edgeEndpoints(edge)[1]==='pollution_active')
    .map(edge=>edgeEndpoints(edge)[0]),['pollution_any_active.y'])
  for(const ahu of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    assert.deepEqual(ahu.connections?.filter(edge=>edgeEndpoints(edge)[1]==='minimum_oa_command.u1')
      .map(edge=>edgeEndpoints(edge)[0]),['minimum_oa_direct_base_select.y'])
    assert.deepEqual(ahu.connections?.filter(edge=>edgeEndpoints(edge)[1]==='minimum_oa_direct_base_select.u2')
      .map(edge=>edgeEndpoints(edge)[0]),['outdoor_air_select.y'])
  }
  assert.ok(migrated.graph.outputs?.some(port=>port.name==='ahu_1_pollution_active'))
  assert.ok(migrated.graph.outputs?.some(port=>port.name==='ahu_2_pollution_level_one_alarm'))
  assert.deepEqual(migrated.layout.ahu_2,{x:70,y:90})
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

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

test('saved G36 AHUs gain direct minimum-air return control while preserving a custom return path', () => {
  const old=structuredClone(reference)
  old.outputs=old.outputs?.filter(port=>!port.name.endsWith('_minimum_oa_return_direct'))
  old.connections=old.connections?.filter(edge=>!edgeEndpoints(edge)[1].endsWith('_minimum_oa_return_direct'))
  for(const ahu of old.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    ahu.outputs=ahu.outputs?.filter(port=>port.name!=='minimum_oa_return_direct')
    ahu.blocks=ahu.blocks?.filter(block=>!['economizer_high_limit_sustained',
      'economizer_high_limit_clear','economizer_high_limit_clear_sustained',
      'economizer_high_limit_lockout','minimum_oa_direct_request',
      'minimum_oa_return_direct_gate','return_damper_direct_select'].includes(block.name))
    ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>
      ['economizer_high_limit_sustained.','economizer_high_limit_clear.',
        'economizer_high_limit_clear_sustained.',
        'economizer_high_limit_lockout.','minimum_oa_direct_request.',
        'minimum_oa_return_direct_gate.','return_damper_direct_select.'].some(prefix=>end.startsWith(prefix))) &&
      edgeEndpoints(edge)[1]!=='minimum_oa_return_direct')
    ahu.connections?.push({from:'return_damper_mode.y',to:'return_damper_physical_limit.u1'})
  }
  const ahu=old.blocks!.find(block=>block.name==='ahu_1')!
  ahu.connections!.find(edge=>edgeEndpoints(edge)[1]==='return_damper_physical_limit.u1')!.from='return_damper_complement.y'
  ahu.blocks!.find(block=>block.name==='return_damper_physical_max')!.parameters![0].value=.82
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:41,y:72}}}
  const migrated=migrateG36PollutionMode(saved,reference)
  for(const current of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    assert.deepEqual(current.connections?.filter(edge=>edgeEndpoints(edge)[1]==='return_damper_physical_limit.u1')
      .map(edge=>edgeEndpoints(edge)[0]),['return_damper_direct_select.y'])
    assert.deepEqual(current.connections?.filter(edge=>edgeEndpoints(edge)[1]==='return_damper_direct_select.u2')
      .map(edge=>edgeEndpoints(edge)[0]),['return_damper_direct_pending_select.y'])
    assert.deepEqual(current.connections?.filter(edge=>edgeEndpoints(edge)[1]==='return_damper_direct_pending_select.u2')
      .map(edge=>edgeEndpoints(edge)[0]),[current.name==='ahu_1'?'return_damper_complement.y':'return_damper_mode.y'])
  }
  assert.equal(migrated.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks
    ?.find(block=>block.name==='return_damper_physical_max')?.parameters?.[0].value,.82)
  assert.deepEqual(migrated.layout.ahu_1,{x:41,y:72})
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

test('saved pollution gates gain a ten-minute CDL release hold without replacing edits',()=>{
  const old=structuredClone(reference)
  for(const ahu of old.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    ahu.blocks=ahu.blocks?.filter(block=>block.name!=='pollution_clear_hold')
    ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>end.startsWith('pollution_clear_hold.')))
    ahu.connections?.push({from:'pollution_clear.y',to:'economizer_pollution_gate.u2'})
  }
  const ahu1=old.blocks!.find(block=>block.name==='ahu_1')!
  ahu1.blocks!.find(block=>block.name==='economizer_mixed_air_pi')!.parameters![0].value=.19
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated=migrateG36PollutionMode(saved,reference)
  for(const ahu of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    assert.deepEqual(ahu.connections?.filter(edge=>edgeEndpoints(edge)[1]==='economizer_pollution_gate.u2')
      .map(edge=>edgeEndpoints(edge)[0]),['pollution_clear_hold.y'])
    assert.deepEqual(ahu.connections?.filter(edge=>edgeEndpoints(edge)[1]==='pollution_clear_hold.u')
      .map(edge=>edgeEndpoints(edge)[0]),['pollution_clear.y'])
  }
  assert.equal(migrated.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks
    ?.find(block=>block.name==='economizer_mixed_air_pi')?.parameters?.[0].value,.19)
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})

test('saved direct-return control gains 15-second and three-minute CDL stages',()=>{
  const old=structuredClone(reference)
  const newNames=['minimum_oa_direct_outdoor_delay','minimum_oa_direct_return_delay','return_damper_direct_pending_select']
  for(const ahu of old.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    ahu.blocks=ahu.blocks?.filter(block=>!newNames.includes(block.name))
    ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>newNames.some(name=>end.startsWith(`${name}.`))))
    ahu.connections?.push({from:'minimum_oa_return_direct_gate.y',to:'minimum_oa_direct_base_select.u3'})
    ahu.connections?.push({from:'minimum_oa_return_direct_gate.y',to:'return_damper_direct_select.u3'})
    ahu.connections?.push({from:'minimum_oa_return_direct_gate.y',to:'minimum_oa_return_direct'})
    ahu.connections?.push({from:ahu.name==='ahu_1'?'return_damper_complement.y':'return_damper_mode.y',to:'return_damper_direct_select.u2'})
  }
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated=migrateG36PollutionMode(saved,reference)
  for(const ahu of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    for(const [blockName,seconds] of [['minimum_oa_direct_outdoor_delay',15],['minimum_oa_direct_return_delay',180]] as const)
      assert.equal(ahu.blocks?.find(block=>block.name===blockName)?.parameters?.find(param=>param.name==='delayTime')?.value,seconds)
    for(const [target,source] of [
      ['minimum_oa_direct_base_select.u3','minimum_oa_direct_outdoor_delay.y'],
      ['return_damper_direct_select.u3','minimum_oa_direct_return_delay.y'],
      ['return_damper_direct_select.u2','return_damper_direct_pending_select.y'],
      ['minimum_oa_return_direct','minimum_oa_direct_return_delay.y'],
      ['return_damper_direct_pending_select.u2',ahu.name==='ahu_1'?'return_damper_complement.y':'return_damper_mode.y'],
    ]) assert.deepEqual(ahu.connections?.filter(edge=>edgeEndpoints(edge)[1]===target).map(edge=>edgeEndpoints(edge)[0]),[source])
  }
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})
