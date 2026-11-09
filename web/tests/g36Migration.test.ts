import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Timing, migrateG36TerminalFeedback, migrateG36AhuFeedback, migrateG36PressureTrim, migrateG36PressureStartupDelay, migrateG36Economizer, migrateG36VacancyDelay } from '../src/lib/g36Migration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block

test('saved G36 pressure startup delay includes the reset sample without replacing custom tuning',()=>{
  const old=structuredClone(reference)
  const delay=(graph:Block,ahu:string)=>graph.blocks?.find(block=>block.name===ahu)?.blocks?.find(block=>block.name==='request_delay')?.parameters?.find(parameter=>parameter.name==='delayTime')
  delay(old,'ahu_1')!.value=600
  delay(old,'ahu_2')!.value=840
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:99,y:123}}}
  const migrated=migrateG36PressureStartupDelay(saved)
  assert.equal(delay(migrated.graph,'ahu_1')?.value,720)
  assert.equal(delay(migrated.graph,'ahu_2')?.value,840)
  assert.equal(delay(old,'ahu_1')?.value,600)
  assert.deepEqual(migrated.layout.ahu_1,{x:99,y:123})
  assert.equal(migrateG36PressureStartupDelay(migrated),migrated)
})

test('shipped G36 vacancy timing becomes five minutes while custom values remain',()=>{
  const old=structuredClone(reference)
  for(const zone of old.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    const delay=zone.blocks?.find(block=>block.name==='vacancy_delay')?.parameters?.find(parameter=>parameter.name==='delayTime')
    assert.ok(delay)
    delay.value=zone.name==='zone_01'?420:120
  }
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{}}
  const migrated=migrateG36VacancyDelay(saved)
  const delay=(name:string)=>migrated.graph.blocks?.find(block=>block.name===name)?.blocks?.find(block=>block.name==='vacancy_delay')?.parameters?.find(parameter=>parameter.name==='delayTime')?.value
  assert.equal(delay('zone_01'),420)
  assert.equal(delay('zone_02'),300)
  assert.equal(migrateG36VacancyDelay(migrated),migrated)
  assert.equal(old.blocks?.find(block=>block.name==='zone_02')?.blocks?.find(block=>block.name==='vacancy_delay')?.parameters?.find(parameter=>parameter.name==='delayTime')?.value,120)
})

test('legacy G36 timing migrates into editable blocks while retaining existing edits',()=>{
  const old=structuredClone(reference)
  old.inputs=old.inputs?.filter(port=>port.name!=='seconds_until_occupied')
  old.inputs?.push({name:'preconditioning_mode',type:'Boolean'})
  old.outputs=old.outputs?.filter(port=>port.name!=='preconditioning_mode'&&!port.name.endsWith('_occupancy_confirmed'))
  old.blocks=old.blocks?.filter(block=>!['preconditioning_horizon','within_preconditioning_horizon','schedule_unoccupied','preconditioning_enabled'].includes(block.name))
  old.connections=old.connections?.filter(edge=>{
    const [from,to]=edgeEndpoints(edge)
    return !['seconds_until_occupied','preconditioning_horizon.y','within_preconditioning_horizon.y','schedule_unoccupied.y'].includes(from)
      &&to!=='preconditioning_mode'&&!to.endsWith('_occupancy_confirmed')
  }).map(edge=>edgeEndpoints(edge)[0]==='preconditioning_enabled.y'?{...edge,from:'preconditioning_mode'}:edge)
  for(let index=1;index<=10;index++){
    const name=`zone_${String(index).padStart(2,'0')}`
    const zone=old.blocks?.find(block=>block.name===name)!
    zone.inputs=zone.inputs?.map(port=>port.name==='zone_presence_raw'?{...port,name:'zone_occupied'}:port)
    zone.outputs=zone.outputs?.filter(port=>port.name!=='occupancy_confirmed')
    zone.blocks=zone.blocks?.filter(block=>!['presence_delay','presence_inverse','vacancy_delay','occupancy_latch'].includes(block.name))
    zone.connections=zone.connections?.filter(edge=>{
      const [from,to]=edgeEndpoints(edge)
      return !['zone_presence_raw','presence_inverse.y','presence_delay.y','vacancy_delay.y'].includes(from)&&to!=='occupancy_confirmed'
    }).map(edge=>edgeEndpoints(edge)[0]==='occupancy_latch.y'?{...edge,from:'zone_occupied'}:edge)
    old.connections=old.connections?.map(edge=>edgeEndpoints(edge)[1]===`${name}.zone_presence_raw`?{...edge,to:`${name}.zone_occupied`}:edge)
  }
  const zone=old.blocks?.find(block=>block.name==='zone_01')!
  zone.notes='Keep my notes'
  const coolSetpoint=zone.blocks?.find(block=>block.name==='cool_setpoint')
  assert.ok(coolSetpoint?.parameters?.[0])
  coolSetpoint.parameters[0].value=23.2
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{zone_01:{x:123,y:456}}}
  const migrated=migrateG36Timing(saved,reference)
  assert.equal(migrated.graph.inputs?.some(port=>port.name==='preconditioning_mode'),false)
  assert.ok(migrated.graph.outputs?.some(port=>port.name==='preconditioning_mode'))
  const updated=migrated.graph.blocks?.find(block=>block.name==='zone_01')!
  assert.equal(updated.notes,'Keep my notes')
  assert.equal(updated.blocks?.find(block=>block.name==='cool_setpoint')?.parameters?.[0].value,23.2)
  assert.ok(updated.blocks?.some(block=>block.name==='occupancy_latch'))
  assert.ok(updated.connections?.some(edge=>edgeEndpoints(edge).join('→')==='zone_presence_raw→presence_delay.u'))
  assert.deepEqual(migrated.layout.zone_01,{x:123,y:456})
  assert.equal(migrateG36Timing(migrated,reference),migrated)
})

test('saved G36 projects gain terminal sensors, limits and alarms without replacing edits',()=>{
  const old=structuredClone(reference)
  old.inputs=old.inputs?.filter(port=>!/_measured_airflow$|_discharge_temp$/.test(port.name))
  old.outputs=old.outputs?.filter(port=>!/_low_airflow_alarm$|_high_discharge_alarm$/.test(port.name))
  old.connections=old.connections?.filter(edge=>!/_measured_airflow$|_discharge_temp$|_low_airflow_alarm$|_high_discharge_alarm$/.test(edgeEndpoints(edge).join(' ')))
  const names=new Set(['maximum_discharge_temp','discharge_headroom','reheat_capacity_per_flow','reheat_flow_cap','reheat_cap_limit','reheat_capped','low_flow_fraction','low_flow_threshold','low_flow_detected','flow_alarm_enable_threshold','flow_requested','low_flow_condition','low_flow_delay','high_discharge_detected','high_discharge_delay','heating_damper_gain','heating_damper_offset','heating_damper_limit','heating_damper_select'])
  for(const zone of old.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    zone.inputs=zone.inputs?.filter(port=>!['measured_airflow','discharge_temp'].includes(port.name))
    zone.outputs=zone.outputs?.filter(port=>!['low_airflow_alarm','high_discharge_alarm'].includes(port.name))
    zone.blocks=zone.blocks?.filter(block=>!names.has(block.name))
    zone.connections=zone.connections?.filter(edge=>{
      const [from,to]=edgeEndpoints(edge)
      return !names.has(from.split('.')[0])&&!names.has(to.split('.')[0])&&!['measured_airflow','discharge_temp','low_airflow_alarm','high_discharge_alarm'].includes(from)&&!['measured_airflow','discharge_temp','low_airflow_alarm','high_discharge_alarm'].includes(to)
    })
    zone.connections?.push({from:'reheat_select.y',to:'reheat_valve_position'})
    zone.connections?.push({from:'damper_select.y',to:'damper_position'},{from:'damper_select.y',to:'flow_span.u'})
  }
  const zone=old.blocks?.find(block=>block.name==='zone_01')!
  zone.notes='Keep my terminal note'
  const heatPi=zone.blocks?.find(block=>block.name==='heat_pi')
  assert.ok(heatPi?.parameters?.[0])
  heatPi.parameters[0].value=0.7
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{zone_01:{x:123,y:456}}}
  const migrated=migrateG36TerminalFeedback(saved,reference)
  const updated=migrated.graph.blocks?.find(block=>block.name==='zone_01')!
  assert.equal(updated.notes,'Keep my terminal note')
  assert.equal(updated.blocks?.find(block=>block.name==='heat_pi')?.parameters?.[0].value,0.7)
  assert.equal(updated.connections?.filter(edge=>edgeEndpoints(edge)[1]==='reheat_valve_position').length,1)
  assert.ok(updated.connections?.some(edge=>edgeEndpoints(edge).join('→')==='reheat_capped.y→reheat_valve_position'))
  assert.ok(updated.connections?.some(edge=>edgeEndpoints(edge).join('→')==='heating_damper_select.y→damper_position'))
  assert.ok(migrated.graph.inputs?.some(port=>port.name==='zone_01_measured_airflow'))
  assert.ok(migrated.graph.outputs?.some(port=>port.name==='zone_10_low_airflow_alarm'))
  assert.deepEqual(migrated.layout.zone_01,{x:123,y:456})
  assert.equal(migrateG36TerminalFeedback(migrated,reference),migrated)
})

test('saved G36 projects gain AHU feedback while preserving custom values',()=>{
  const old=structuredClone(reference)
  const ahu=old.blocks?.find(block=>block.name==='ahu_1')!
  ahu.blocks=ahu.blocks?.filter(block=>!['fan_enable','duct_pressure_pi','minimum_oa_pi'].includes(block.name))
  ahu.blocks!.find(block=>block.name==='fan_gain')!.parameters![0].value=0.16
  old.inputs=old.inputs?.filter(port=>port.name!=='ahu_1_outdoor_airflow')
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:231,y:432}}}
  const migrated=migrateG36AhuFeedback(saved,reference)
  const updated=migrated.graph.blocks?.find(block=>block.name==='ahu_1')!
  assert.equal(updated.blocks?.find(block=>block.name==='fan_gain')?.parameters?.[0].value,0.16)
  assert.ok(updated.blocks?.some(block=>block.name==='duct_pressure_pi'))
  assert.ok(updated.blocks?.some(block=>block.name==='minimum_oa_pi'))
  assert.ok(migrated.graph.inputs?.some(port=>port.name==='ahu_1_outdoor_airflow'))
  assert.deepEqual(migrated.layout.ahu_1,{x:231,y:432})
  assert.equal(migrateG36AhuFeedback(migrated,reference),migrated)
})

test('saved G36 pressure reset migrates to sampled trim and respond',()=>{
  const old=structuredClone(reference)
  const ahu=old.blocks?.find(block=>block.name==='ahu_1')!
  ahu.blocks=ahu.blocks?.filter(block=>block.name!=='pressure_previous')
  ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge).some(end=>end.startsWith('pressure_previous.')))
  ahu.blocks!.find(block=>block.name==='response_per_request')!.parameters![0].value=0.08
  ahu.blocks!.push({name:'pressure_target_select',block_type:'Switch',inputs:[{name:'u1',type:'Real'},{name:'u2',type:'Real'},{name:'u3',type:'Boolean'}],outputs:[{name:'y',type:'Real'}]})
  ahu.connections=ahu.connections?.filter(edge=>!['duct_pressure_setpoint','duct_pressure_pi.u_s'].includes(edgeEndpoints(edge)[1]))
  ahu.connections?.push({from:'high_pressure_target.y',to:'pressure_target_select.u1'},{from:'low_pressure_target.y',to:'pressure_target_select.u2'},{from:'request_delay.y',to:'pressure_target_select.u3'},{from:'pressure_target_select.y',to:'duct_pressure_setpoint'},{from:'pressure_target_select.y',to:'duct_pressure_pi.u_s'})
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:222,y:333}}}
  const migrated=migrateG36PressureTrim(saved,reference)
  const updated=migrated.graph.blocks?.find(block=>block.name==='ahu_1')!
  assert.ok(updated.blocks?.some(block=>block.name==='pressure_previous'))
  assert.equal(updated.blocks?.find(block=>block.name==='response_per_request')?.parameters?.[0].value,0.08)
  assert.equal(updated.connections?.filter(edge=>edgeEndpoints(edge)[1]==='duct_pressure_setpoint').length,1)
  assert.ok(updated.connections?.some(edge=>edgeEndpoints(edge).join('→')==='pressure_candidate.y→pressure_previous.u'))
  assert.deepEqual(migrated.layout.ahu_1,{x:222,y:333})
  assert.equal(migrateG36PressureTrim(migrated,reference),migrated)
})

test('saved G36 economizer gains editable mixed-air feedback without losing the old maximum',()=>{
  const old=structuredClone(reference)
  old.inputs=old.inputs?.filter(port=>!port.name.endsWith('_mixed_air_temp'))
  old.connections=old.connections?.filter(edge=>!edgeEndpoints(edge)[0].endsWith('_mixed_air_temp'))
  for(const ahu of old.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    ahu.inputs=ahu.inputs?.filter(port=>port.name!=='mixed_air_temp')
    ahu.blocks=ahu.blocks?.filter(block=>block.name!=='economizer_mixed_air_pi')
    ahu.blocks?.push({name:'free_cooling_outdoor_air',block_type:'Constant',inputs:[],outputs:[{name:'y',type:'Real'}],parameters:[{name:'k',type:'Real',value:ahu.name==='ahu_1'?0.7:0.8}]})
    ahu.blocks?.push({name:'outdoor_above_supply_target',block_type:'Greater',inputs:[{name:'u1',type:'Real'},{name:'u2',type:'Real'}],outputs:[{name:'y',type:'Boolean'}]})
    ahu.connections=ahu.connections?.filter(edge=>!['economizer_mixed_air_pi','mixed_air_temp'].some(name=>edgeEndpoints(edge).join(' ').includes(name))
      &&!['economizer_temperature_ok.u2','outdoor_air_select.u1'].includes(edgeEndpoints(edge)[1]))
    ahu.connections?.push({from:'free_cooling_outdoor_air.y',to:'outdoor_air_select.u1'},
      {from:'outdoor_c',to:'outdoor_above_supply_target.u1'},
      {from:'supply_reset.y',to:'outdoor_above_supply_target.u2'},
      {from:'outdoor_above_supply_target.y',to:'economizer_temperature_ok.u2'})
  }
  const saved:Project={version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:231,y:432}}}
  const migrated=migrateG36Economizer(saved,reference)
  const first=migrated.graph.blocks?.find(block=>block.name==='ahu_1')!
  const second=migrated.graph.blocks?.find(block=>block.name==='ahu_2')!
  assert.equal(first.blocks?.find(block=>block.name==='economizer_mixed_air_pi')?.parameters?.find(parameter=>parameter.name==='yMax')?.value,0.7)
  assert.equal(second.blocks?.find(block=>block.name==='economizer_mixed_air_pi')?.parameters?.find(parameter=>parameter.name==='yMax')?.value,0.8)
  assert.ok(migrated.graph.inputs?.some(port=>port.name==='ahu_1_mixed_air_temp'))
  assert.ok(first.connections?.some(edge=>edgeEndpoints(edge).join('→')==='mixed_air_temp→economizer_mixed_air_pi.u_m'))
  assert.equal(first.connections?.filter(edge=>edgeEndpoints(edge)[1]==='outdoor_air_select.u1').length,1)
  assert.deepEqual(migrated.layout.ahu_1,{x:231,y:432})
  assert.equal(migrateG36Economizer(migrated,reference),migrated)
})
