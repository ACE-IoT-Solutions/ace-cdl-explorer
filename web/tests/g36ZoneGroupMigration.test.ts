import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ZoneGroups } from '../src/lib/g36ZoneGroupMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const isGroup = (name: string) => name === 'any_group_preconditioning' || /^zone_group_[12]$/.test(name) || /^zone_\d{2}_(heat|cool)_target_previous$/.test(name)

test('saved G36 graph gains sampled five-zone groups and retains zone tuning', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks!.filter(block => !isGroup(block.name))
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_group_/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !isGroup(from.split('.')[0]) && !isGroup(to.split('.')[0]) &&
      !/\.group_(warmup|cooldown)$/.test(to) && !/^ahu_[12]_group_/.test(to)
  })
  old.connections.push({ from: 'preconditioning_enabled.y', to: 'preconditioning_mode' })
  for (let index = 1; index <= 10; index++) {
    const zone = old.blocks.find(block => block.name === `zone_${String(index).padStart(2, '0')}`)!
    zone.inputs = zone.inputs!.filter(port => !['group_warmup', 'group_cooldown'].includes(port.name))
    zone.inputs.push({ name: 'preconditioning_mode', type: 'Boolean' })
    zone.outputs = zone.outputs!.filter(port => !['occupied_heat_target', 'occupied_cool_target'].includes(port.name))
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !['group_warmup', 'group_cooldown'].includes(from) &&
        !['occupied_heat_target', 'occupied_cool_target'].includes(to)
    })
    zone.connections.push({ from: 'preconditioning_mode', to: 'warmup_mode.u1' })
    zone.connections.push({ from: 'preconditioning_mode', to: 'cooldown_mode.u1' })
    old.connections.push({ from: 'preconditioning_enabled.y', to: `${zone.name}.preconditioning_mode` })
  }
  old.blocks.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  old.blocks.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value = 16
  old.blocks.find(block => block.name === 'zone_02')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value = 19
  old.blocks.find(block => block.name === 'preconditioning_horizon')!.parameters![0].value = 5400
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 42, y: 24 } } }
  const next = migrateG36ZoneGroups(saved, reference)
  assert.equal(next.graph.blocks!.filter(block => /^zone_group_[12]$/.test(block.name)).length, 2)
  assert.equal(next.graph.blocks!.filter(block => /^zone_\d{2}_(heat|cool)_target_previous$/.test(block.name)).length, 20)
  assert.equal(next.graph.outputs!.filter(port => /^ahu_[12]_group_/.test(port.name)).length, 6)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value, 18)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_02')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value, 19)
  assert.equal(next.graph.blocks!.find(block => block.name === 'preconditioning_horizon')!.parameters![0].value, 5400)
  assert.deepEqual(next.layout.zone_01, { x: 42, y: 24 })
  assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'preconditioning_mode').length, 1)
  assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'zone_01.group_warmup').length, 1)
  assert.equal(migrateG36ZoneGroups(next, reference), next)
})
