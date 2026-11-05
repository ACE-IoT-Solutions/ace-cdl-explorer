import assert from 'node:assert/strict'
import { test } from 'node:test'
import { blockCatalog } from '../src/lib/blockCatalog'
import { blockEquations } from '../src/lib/blockHelp'
import type { Block } from '../src/lib/types'

test('every supported elementary CDL block has a displayed formula',()=>{
  for(const definition of blockCatalog.filter(item=>item.group==='Elementary blocks')){
    assert.ok(definition.equations?.length,`${definition.type} has no equation`)
    assert.equal(blockEquations({name:'sample',block_type:definition.type})[0],definition.equations![0])
  }
})

test('imported equations override the standard formula',()=>{
  const block:Block={name:'custom',block_type:'CDL.Reals.Min',equations:[{lhs:'y',rhs:'min(u1, u2) + offset'}]}
  assert.deepEqual(blockEquations(block),['y = min(u1, u2) + offset'])
})

test('imported CDL Switch uses its Boolean u2 connector',()=>{
  const block:Block={name:'choice',block_type:'CDL.Logical.Switch',inputs:[{name:'u1',type:'Real'},{name:'u2',type:'Boolean'},{name:'u3',type:'Real'}]}
  assert.deepEqual(blockEquations(block),['y = u2 ? u1 : u3'])
})
