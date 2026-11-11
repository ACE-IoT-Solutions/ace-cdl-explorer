import assert from 'node:assert/strict'
import { test } from 'node:test'
import { maxRunHistory, readRunHistory, runHistoryKey, saveRunHistory, type SimulationRun } from '../src/lib/runHistory'

const makeRun = (n:number):SimulationRun => ({
  id:String(n), program:n%2?'g36':'conventional', label:n%2?'G36-style controls':'Conventional controls',
  completedAt:new Date(Date.UTC(2026,10,9,12,n)).toISOString(), durationMs:1000+n,
  requestedStepSeconds:900, effectiveStepSeconds:60,
  summary:{electricity_kwh:n,gas_kwh:n+1,unmet_zone_hours:n+2,occupied_unmet_zone_hours:n+3},
})

test('run history persists only completed summary records in newest-first order', () => {
  const entries = new Map<string,string>()
  const storage = {getItem:(key:string)=>entries.get(key)??null,setItem:(key:string,value:string)=>{entries.set(key,value)}}
  const runs = Array.from({length:maxRunHistory+3},(_,index)=>makeRun(index))
  assert.equal(saveRunHistory(storage,runs),true)
  assert.deepEqual(readRunHistory(storage).map(run=>run.id),runs.slice(0,maxRunHistory).map(run=>run.id))
  const saved = JSON.parse(entries.get(runHistoryKey)!)
  assert.deepEqual(saved[0].summary,runs[0].summary)
  assert.deepEqual(readRunHistory(storage)[0].summary,runs[0].summary)
  assert.equal(saved[0].rows,undefined)
})

test('run history ignores malformed and incomplete stored runs', () => {
  const storage = {getItem:()=>JSON.stringify([makeRun(1),{...makeRun(2),summary:{electricity_kwh:8}},null])}
  assert.deepEqual(readRunHistory(storage).map(run=>run.id),['1'])
  assert.deepEqual(readRunHistory({getItem:()=>'{broken'}),[])
})

test('run history preserves program identity after a run is renamed and upgrades older records', () => {
  const renamed = {...makeRun(1),label:'Fan gain 0.1'}
  const legacy = {...makeRun(3),program:undefined}
  const storage = {getItem:()=>JSON.stringify([renamed,legacy])}
  assert.deepEqual(readRunHistory(storage).map(run=>run.program),['g36','g36'])
})
