import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Overrides } from '../src/lib/g36OverrideMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const addedGroup = (name: string) => /^(override_any_|group_occupied$|not_group_occupied$|lookahead_unoccupied$|window_any_|window_alarm_)/.test(name)
const wires = (block: Block) => new Set((block.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))

test('saved G36 projects gain local override and window alarm CDL while retaining tuning', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => !/^zone_\d{2}_override$/.test(port.name))
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_group_(occupied|window_open_alarm_level)$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from,to] = edgeEndpoints(edge)
    return !from.endsWith('_override') && !/^zone_group_[12]\.(occupied|window_open_alarm_level)$/.test(from)
  })
  for (const group of old.blocks ?? []) {
    const isGroup = /^zone_group_[12]$/.test(group.name)
    if (!isGroup && !/^zone_\d{2}$/.test(group.name)) continue
    const isNew = isGroup ? addedGroup : (name:string) => name === 'local_occupied'
    group.inputs = group.inputs!.filter(port => isGroup ? !/^override_[1-5]$/.test(port.name) : port.name !== 'local_override')
    if (isGroup) group.outputs = group.outputs!.filter(port => !['occupied', 'window_open_alarm_level'].includes(port.name))
    group.blocks = group.blocks!.filter(block => !isNew(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return !isNew(from.split('.')[0]) && !isNew(to.split('.')[0]) &&
        !(isGroup ? /^override_[1-5]$/.test(from) : from === 'local_override') &&
        !['occupied', 'window_open_alarm_level'].includes(to)
    })
  }
  const group = old.blocks!.find(block => block.name === 'zone_group_1')!
  group.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 4800
  const zone = old.blocks!.find(block => block.name === 'zone_01')!
  zone.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved:Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{zone_group_1:{x:30,y:40}}}
  const next=migrateG36Overrides(saved,reference)
  assert.equal(next.graph.inputs!.filter(port => port.name.endsWith('_override')).length,10)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value,4800)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value,22)
  assert.deepEqual(next.layout.zone_group_1,{x:30,y:40})
  for (const name of ['zone_group_1','zone_group_2','zone_01','zone_10']) {
    const actual=next.graph.blocks!.find(block => block.name === name)!
    const template=reference.blocks!.find(block => block.name === name)!
    for (const wire of wires(template)) assert.ok(wires(actual).has(wire),`${name}: ${wire}`)
  }
  assert.equal(migrateG36Overrides(next,reference),next)
})
