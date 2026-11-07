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

test('G36 trigger period and shift refine the displayed internal step',()=>{
  const edited=structuredClone(g36)
  for(const group of edited.blocks?.filter(block=>block.name.startsWith('zone_group_'))??[]){
    const clock=group.blocks?.find(block=>block.name==='optimal_daily_sample')
    assert.ok(clock)
    clock.parameters!.find(parameter=>parameter.name==='period')!.value=45
    clock.parameters!.find(parameter=>parameter.name==='shift')!.value=15
  }
  assert.equal(effectiveStepForGraph(edited,900),15)
  for(const group of edited.blocks?.filter(block=>block.name.startsWith('zone_group_'))??[]){
    const clock=group.blocks!.find(block=>block.name==='optimal_daily_sample')!
    clock.parameters!.find(parameter=>parameter.name==='period')!.value=60
    clock.parameters!.find(parameter=>parameter.name==='shift')!.value=30
  }
  assert.equal(effectiveStepForGraph(edited,900),30)
})
