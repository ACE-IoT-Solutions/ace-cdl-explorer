import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Timing, migrateG36TerminalFeedback, migrateG36AhuFeedback, migrateG36PressureTrim } from '../src/lib/g36Migration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block

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
