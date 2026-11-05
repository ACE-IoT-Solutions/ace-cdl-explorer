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
