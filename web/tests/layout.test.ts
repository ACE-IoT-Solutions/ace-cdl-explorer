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
