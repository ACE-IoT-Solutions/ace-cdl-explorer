import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {test} from 'node:test'
import {migrateG36MinimumOutdoorAirPi, needsG36MinimumOutdoorAirPiMigration} from '../src/lib/g36MinimumOutdoorAirPiMigration'
import {defaultScenario, edgeEndpoints, type Block, type Project} from '../src/lib/types'

const reference=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block

test('saved common-damper loops gain normalized CDL feedback and fan-proof reset',()=>{
  const graph=structuredClone(reference)
  graph.name='Edited G36 building'
  for(const ahu of graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    ahu.blocks=ahu.blocks?.filter(block=>!['minimum_oa_design_flow','minimum_oa_design_floor','minimum_oa_design_safe','minimum_oa_setpoint_normalized','minimum_oa_measured_normalized'].includes(block.name))
    const pi=ahu.blocks!.find(block=>block.name==='minimum_oa_pi')!
    pi.block_type='LimPID'
    pi.inputs=pi.inputs?.filter(port=>port.name!=='trigger')
    pi.parameters=[{name:'k',value:ahu.name==='ahu_1'?0.3:0.2},{name:'Ti',value:ahu.name==='ahu_1'?900:3600},{name:'yMin',value:0},{name:'yMax',value:1},{name:'reverseActing',value:true}]
    pi.notes=ahu.name==='ahu_1'?'My outdoor air loop':'Increase outside-air damper command when measured flow misses its minimum setpoint.'
    ahu.connections=ahu.connections?.filter(edge=>{
      const [from,to]=edgeEndpoints(edge)
      return !from.startsWith('minimum_oa_design_')&&!from.startsWith('minimum_oa_setpoint_normalized.')&&!from.startsWith('minimum_oa_measured_normalized.')
        && !to.startsWith('minimum_oa_design_')&&!to.startsWith('minimum_oa_setpoint_normalized.')&&!to.startsWith('minimum_oa_measured_normalized.')
        && to!=='minimum_oa_pi.trigger'
    })
    ahu.connections?.push({from:'minimum_oa_setpoint.y',to:'minimum_oa_pi.u_s'},{from:'outdoor_airflow',to:'minimum_oa_pi.u_m'})
  }
  const project:Project={version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{ahu_1:{x:17,y:23}}}
  assert.equal(needsG36MinimumOutdoorAirPiMigration(project),true)
  const migrated=migrateG36MinimumOutdoorAirPi(project,reference)
  for(const ahu of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    const pi=ahu.blocks?.find(block=>block.name==='minimum_oa_pi')!
    const param=(name:string)=>pi.parameters?.find(item=>item.name===name)?.value
    assert.equal(pi.block_type,'Buildings.Controls.OBC.CDL.Reals.PIDWithReset')
    assert.ok(pi.inputs?.some(port=>port.name==='trigger'&&port.type==='Boolean'))
    assert.equal(param('k'),ahu.name==='ahu_1'?0.3*0.75:0.05)
    assert.equal(param('Ti'),ahu.name==='ahu_1'?900:120)
    assert.equal(pi.notes,ahu.name==='ahu_1'?'My outdoor air loop':reference.blocks?.find(block=>block.name===ahu.name)?.blocks?.find(block=>block.name==='minimum_oa_pi')?.notes)
    const edges=new Set(ahu.connections?.map(edge=>edgeEndpoints(edge).join('→')))
    for(const edge of ['vent_effective_limit.y→minimum_oa_setpoint_normalized.u1','minimum_oa_setpoint_normalized.y→minimum_oa_pi.u_s','minimum_oa_measured_normalized.y→minimum_oa_pi.u_m','supply_fan_proven→minimum_oa_pi.trigger']) assert.ok(edges.has(edge))
    assert.ok(!edges.has('minimum_oa_setpoint.y→minimum_oa_pi.u_s'))
    assert.ok(!edges.has('outdoor_airflow→minimum_oa_pi.u_m'))
  }
  assert.deepEqual(migrated.layout.ahu_1,{x:17,y:23})
  assert.equal(migrateG36MinimumOutdoorAirPi(migrated,reference),migrated)
  assert.equal(project.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.find(block=>block.name==='minimum_oa_pi')?.block_type,'LimPID')
})

test('saved normalized loop keeps PI tuning while removing the setpoint gate ahead of it',()=>{
  const graph=structuredClone(reference)
  graph.name='Edited G36 building'
  for(const ahu of graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    const edge=ahu.connections?.find(edge=>edgeEndpoints(edge).join('→')==='vent_effective_limit.y→minimum_oa_setpoint_normalized.u1')
    assert.ok(edge)
    edge.from='minimum_oa_setpoint.y'
    const gain=ahu.blocks?.find(block=>block.name==='minimum_oa_pi')?.parameters?.find(parameter=>parameter.name==='k')
    assert.ok(gain)
    gain.value=0.13
  }
  const project:Project={version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{ahu_1:{x:17,y:23}}}
  assert.equal(needsG36MinimumOutdoorAirPiMigration(project),true)
  const migrated=migrateG36MinimumOutdoorAirPi(project,reference)
  for(const ahu of migrated.graph.blocks?.filter(block=>block.block_type==='AHUController')??[]){
    const edges=new Set(ahu.connections?.map(edge=>edgeEndpoints(edge).join('→')))
    assert.ok(edges.has('vent_effective_limit.y→minimum_oa_setpoint_normalized.u1'))
    assert.ok(!edges.has('minimum_oa_setpoint.y→minimum_oa_setpoint_normalized.u1'))
    assert.equal(ahu.blocks?.find(block=>block.name==='minimum_oa_pi')?.parameters?.find(parameter=>parameter.name==='k')?.value,0.13)
  }
  assert.deepEqual(migrated.layout.ahu_1,{x:17,y:23})
  assert.equal(needsG36MinimumOutdoorAirPiMigration(migrated),false)
  assert.equal(migrateG36MinimumOutdoorAirPi(migrated,reference),migrated)
})

test('nonstandard edited PI block is left for manual reconciliation',()=>{
  const graph=structuredClone(reference)
  const ahu=graph.blocks!.find(block=>block.name==='ahu_1')!
  const pi=ahu.blocks!.find(block=>block.name==='minimum_oa_pi')!
  pi.block_type='CustomPI'
  ahu.connections=ahu.connections?.filter(edge=>!edgeEndpoints(edge)[1].startsWith('minimum_oa_pi.'))
  ahu.connections?.push({from:'minimum_oa_setpoint.y',to:'minimum_oa_pi.u_s'},{from:'outdoor_airflow',to:'minimum_oa_pi.u_m'})
  const project:Project={version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{}}
  assert.equal(needsG36MinimumOutdoorAirPiMigration(project),false)
  assert.equal(migrateG36MinimumOutdoorAirPi(project,reference),project)
})
