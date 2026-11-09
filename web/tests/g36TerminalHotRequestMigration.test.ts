import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TerminalHotRequestLevels } from '../src/lib/g36TerminalHotRequestLevelsMigration'
import { edgeEndpoints, layoutFor, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 teaching requests upgrade to editable OBC tiers without replacing plant tuning', () => {
  const old=structuredClone(reference)
  const plant=old.blocks!.find(block=>block.name==='plant')!
  const newPlant=(name:string)=>name.startsWith('terminal_hot_reset_sum_')||name.startsWith('terminal_hot_plant_sum_')||['terminal_hot_plant_request_active','hot_plant_request_or_heat','hot_reset_all_request_count'].includes(name)
  plant.blocks=plant.blocks!.filter(block=>!newPlant(block.name))
  plant.connections=plant.connections!.filter(edge=>{const [from,to]=edgeEndpoints(edge);return !newPlant(from.split('.')[0])&&!newPlant(to.split('.')[0])})
  plant.inputs=plant.inputs!.filter(port=>!/^zone_\d\d_hot_water_(reset|plant)_request$/.test(port.name))
  plant.outputs=plant.outputs!.filter(port=>port.name!=='terminal_hot_water_plant_request_count')
  plant.connections.push({from:'hot_reset_request_count.y',to:'hot_reset_request_real.u'})
  plant.connections.push({from:'hot_water_pump_needed.y',to:'freeze_hot_water_pump_enable.u1'})
  plant.blocks.push({name:'terminal_heat_demand_threshold',block_type:'Constant',category:'elementary',inputs:[],outputs:[{name:'y',type:'Real'}],parameters:[{name:'k',value:0.5}]})
  plant.notes='My edited plant notes'
  plant.blocks.find(block=>block.name==='hot_reset_warm_delta')!.parameters![0].value=-7.0
  for(let i=1;i<=10;i++){
    const name=`zone_${String(i).padStart(2,'0')}`
    const zone=old.blocks!.find(block=>block.name===name)!
    const newZone=(blockName:string)=>blockName.startsWith('hot_reset_')||blockName.startsWith('hot_plant_')
    zone.blocks=zone.blocks!.filter(block=>!newZone(block.name))
    zone.connections=zone.connections!.filter(edge=>{const [from,to]=edgeEndpoints(edge);return !newZone(from.split('.')[0])&&!newZone(to.split('.')[0])&&to!=='hot_water_reset_request'&&to!=='hot_water_plant_request'})
    zone.outputs=zone.outputs!.filter(port=>!['hot_water_reset_request','hot_water_plant_request'].includes(port.name))
    for(const suffix of ['hot_water_reset_request','hot_water_plant_request'])old.outputs=old.outputs!.filter(port=>port.name!==`${name}_${suffix}`)
  }
  old.outputs=old.outputs!.filter(port=>port.name!=='plant_terminal_hot_water_plant_request_count')
  old.connections=old.connections!.filter(edge=>{const [from,to]=edgeEndpoints(edge);return !/^zone_\d\d\.hot_water_(reset|plant)_request$/.test(from)&&from!=='plant.terminal_hot_water_plant_request_count'&&!/^plant\.zone_\d\d_hot_water_(reset|plant)_request$/.test(to)})
  old.connections.push({from:'zone_01.heating_demand',to:'plant.zone_01_heating_demand'})
  plant.inputs!.push({name:'zone_01_heating_demand',type:'Real'})
  const saved:Project={version:1,name:old.name,graph:old,layout:layoutFor(old),scenario:{} as Project['scenario']}
  const migrated=migrateG36TerminalHotRequestLevels(saved,reference)
  const upgraded=migrated.graph.blocks!.find(block=>block.name==='plant')!
  assert.equal(upgraded.notes,'My edited plant notes')
  assert.equal(upgraded.blocks!.find(block=>block.name==='hot_reset_warm_delta')!.parameters![0].value,-7.0)
  assert.ok(migrated.graph.blocks!.find(block=>block.name==='zone_01')!.blocks!.some(block=>block.name==='hot_reset_three_requests'))
  assert.equal(upgraded.connections!.filter(edge=>edgeEndpoints(edge)[1]==='hot_reset_request_real.u').length,1)
  assert.equal(upgraded.connections!.filter(edge=>edgeEndpoints(edge)[1]==='freeze_hot_water_pump_enable.u1').length,1)
  assert.equal(migrated.graph.connections!.filter(edge=>edgeEndpoints(edge)[1]==='plant.zone_01_heating_demand').length,0)
  assert.equal(migrateG36TerminalHotRequestLevels(migrated,reference),migrated)
})
