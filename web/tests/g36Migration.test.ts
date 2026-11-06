import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Timing } from '../src/lib/g36Migration'
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
