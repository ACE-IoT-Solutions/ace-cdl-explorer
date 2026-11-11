import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { effectiveStepForGraph, type Block } from '../src/lib/types'

const g36=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block
const conventional=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8')) as Block

test('G36 sampled CDL blocks refine the requested maximum timestep',()=>{
  assert.equal(effectiveStepForGraph(g36,900),15)
  assert.equal(effectiveStepForGraph(g36,300),15)
  assert.equal(effectiveStepForGraph(g36,60),15)
  assert.equal(effectiveStepForGraph(g36,15),15)
  assert.equal(effectiveStepForGraph(conventional,900),900)
})

test('G36 trigger period and shift refine the displayed internal step',()=>{
  const edited=structuredClone(g36)
  const relaxFastDelays=(block:Block)=>{
    if(block.block_type.split('.').at(-1)==='TrueDelay'){
      const delay=block.parameters?.find(parameter=>parameter.name==='delayTime')
      if(delay?.value===15)delay.value=60
    }
    for(const child of block.blocks??[])relaxFastDelays(child)
  }
  relaxFastDelays(edited)
  for(const group of edited.blocks?.filter(block=>block.name.startsWith('zone_group_'))??[]){
    const clock=group.blocks?.find(block=>block.name==='optimal_warmup_1_obc')?.blocks?.find(block=>block.name==='samTri')
    assert.ok(clock)
    clock.parameters!.find(parameter=>parameter.name==='period')!.value=45
    clock.parameters!.find(parameter=>parameter.name==='shift')!.value=15
  }
  assert.equal(effectiveStepForGraph(edited,900),15)
  for(const group of edited.blocks?.filter(block=>block.name.startsWith('zone_group_'))??[]){
    const clock=group.blocks!.find(block=>block.name==='optimal_warmup_1_obc')!.blocks!.find(block=>block.name==='samTri')!
    clock.parameters!.find(parameter=>parameter.name==='period')!.value=60
    clock.parameters!.find(parameter=>parameter.name==='shift')!.value=30
  }
  assert.equal(effectiveStepForGraph(edited,900),30)
})

test('a tuned periodic sampler participates in the internal control step',()=>{
  const edited=structuredClone(g36)
  for(const zone of edited.blocks?.filter(block=>block.block_type==='VAVBoxController')??[]){
    zone.blocks!.find(block=>block.name==='sat_sampled_cooling_setpoint')!
      .parameters!.find(parameter=>parameter.name==='samplePeriod')!.value=45
  }
  assert.equal(effectiveStepForGraph(edited,900),15)
})

test('timed CDL holds refine the displayed internal step',()=>{
  const graph:Block={name:'timed',block_type:'Composite',blocks:[
    {name:'delay',block_type:'TrueDelay',parameters:[{name:'delayTime',value:30}]},
    {name:'hold',block_type:'TrueFalseHold',parameters:[
      {name:'trueHoldDuration',value:300},{name:'falseHoldDuration',value:0},
    ]},
  ]}
  assert.equal(effectiveStepForGraph(graph,900),30)
  graph.blocks![1].parameters![0].value=75
  assert.equal(effectiveStepForGraph(graph,900),15)
})
