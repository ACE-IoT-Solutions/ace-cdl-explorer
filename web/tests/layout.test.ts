import assert from 'node:assert/strict'
import test from 'node:test'
import { layoutFor, type Block } from '../src/lib/types'

test('places sources left and sizes columns from dependency depth',()=>{
  const graph:Block={name:'example',block_type:'Composite',category:'composite',blocks:[
    {name:'constant',block_type:'Constant',outputs:[{name:'y',type:'Real'}]},
    {name:'gain',block_type:'MultiplyByParameter',inputs:[{name:'u',type:'Real'}],outputs:[{name:'y',type:'Real'}]},
    {name:'limit',block_type:'Limiter',inputs:[{name:'u',type:'Real'}],outputs:[{name:'y',type:'Real'}]},
  ],connections:[{from:'constant.y',to:'gain.u'},{from:'gain.y',to:'limit.u'}]}
  const layout=layoutFor(graph)
  assert.deepEqual([layout.constant.x,layout.gain.x,layout.limit.x],[60,620,1180])
  assert.equal(layout.constant.y,80)
  assert.equal(layout.gain.y,80)
  assert.equal(layout.limit.y,80)
  assert.equal(layoutFor(graph,'previous').limit.x,780)
})

test('reserves separate output and input card lanes between building equipment',()=>{
  const names=['plant','ahu_1','zone_01','zone_02','ahu_2','zone_06']
  const graph:Block={name:'building',block_type:'BuildingController',category:'composite',blocks:names.map(name=>({name,block_type:'Composite',category:'composite'}))}
  const layout=layoutFor(graph)
  assert.deepEqual(names.map(name=>layout[name].x),[60,860,1660,1660,2460,3260])
  assert.equal(layout.zone_02.y-layout.zone_01.y,280)
  assert.equal(layoutFor(graph,'previous').zone_01.x,840)
  assert.equal(layoutFor(graph,'depth24').zone_02.y-layoutFor(graph,'depth24').zone_01.y,240)
})

test('places sampled state before the feedback calculations it drives',()=>{
  const graph:Block={name:'reset',block_type:'Composite',category:'composite',blocks:[
    {name:'previous',block_type:'Buildings.Controls.OBC.CDL.Discrete.UnitDelay',inputs:[{name:'u',type:'Real'}],outputs:[{name:'y',type:'Real'}]},
    {name:'sum',block_type:'Add',inputs:[{name:'u1',type:'Real'},{name:'u2',type:'Real'}],outputs:[{name:'y',type:'Real'}]},
    {name:'trim',block_type:'Constant',outputs:[{name:'y',type:'Real'}]},
  ],connections:[{from:'previous.y',to:'sum.u1'},{from:'trim.y',to:'sum.u2'},{from:'sum.y',to:'previous.u'}]}
  const layout=layoutFor(graph)
  assert.ok(layout.previous.x<layout.sum.x)
  assert.ok(layout.trim.x<layout.sum.x)
})
