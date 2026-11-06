import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { effectiveStepForGraph, type Block } from '../src/lib/types'

const g36=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block
const conventional=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8')) as Block

test('G36 sampled CDL blocks refine the requested maximum timestep',()=>{
  assert.equal(effectiveStepForGraph(g36,900),60)
  assert.equal(effectiveStepForGraph(g36,300),60)
  assert.equal(effectiveStepForGraph(g36,60),60)
  assert.equal(effectiveStepForGraph(g36,15),15)
  assert.equal(effectiveStepForGraph(conventional,900),900)
})
