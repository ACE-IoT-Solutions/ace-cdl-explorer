import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Windows } from '../src/lib/g36WindowMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const isWindowBlock = (name: string) => name.startsWith('window_') || name.includes('_window_')
const wires = (block: Block) => new Set((block.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))

test('saved G36 graph gains window gates without losing edited CDL parameters', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs!.filter(port => !port.name.endsWith('_window_closed'))
  old.connections = old.connections!.filter(edge => !edgeEndpoints(edge)[0].endsWith('_window_closed'))
  for (const block of old.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(block.name) && !/^zone_\d{2}$/.test(block.name)) continue
    block.inputs = block.inputs!.filter(port => !port.name.startsWith('window_closed'))
    block.blocks = block.blocks!.filter(child => !isWindowBlock(child.name))
    block.connections = block.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !isWindowBlock(from.split('.')[0]) && !isWindowBlock(to.split('.')[0]) &&
        !from.startsWith('window_closed')
    })
  }
  const oldGroup = old.blocks!.find(block => block.name === 'zone_group_1')!
  oldGroup.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 4100
  const oldZone = old.blocks!.find(block => block.name === 'zone_01')!
  oldZone.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 77, y: 88 } } }
  const next = migrateG36Windows(saved, reference)
  assert.equal(next.graph.inputs!.filter(port => port.name.endsWith('_window_closed')).length, 10)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value, 4100)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_01, { x: 77, y: 88 })
  for (const name of ['zone_group_1', 'zone_group_2', 'zone_01', 'zone_10']) {
    const actual = next.graph.blocks!.find(block => block.name === name)!
    const template = reference.blocks!.find(block => block.name === name)!
    for (const wire of wires(template)) assert.ok(wires(actual).has(wire), `${name}: ${wire}`)
  }
  assert.equal(migrateG36Windows(next, reference), next)
})
