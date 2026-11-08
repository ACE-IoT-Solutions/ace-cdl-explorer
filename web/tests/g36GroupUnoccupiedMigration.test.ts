import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36GroupUnoccupied } from '../src/lib/g36GroupUnoccupiedMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_obc_optimal_start.json', import.meta.url), 'utf8')) as Block
const newRoot = (name: string) => /^zone_\d{2}_unoccupied_(heat|cool)_previous$/.test(name)
const newGroupBlock = (name: string) => /^((cold|hot|warm_end|cool_end|warm_end_target|cool_end_target)_\d|\b(all_|sum_|min_room_)|average_|setback_|setup_|freeze_|schedule_unoccupied_group|not_preconditioning_group|unoccupied_allowed|ordinary_allowed|not_freeze_group|not_setback_group)/.test(name)
const newZoneBlock = (name: string) => ['unoccupied_heat_off', 'unoccupied_cool_off', 'group_heat_allowed', 'unoccupied_heat_enabled', 'unoccupied_cool_enabled', 'mode_freeze_setback', 'mode_select_freeze_setback'].includes(name)
const wireIds = (block: Block) => new Set((block.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
const expectTemplateWires = (actual: Block, template: Block) => {
  const present = wireIds(actual)
  for (const wire of wireIds(template)) assert.ok(present.has(wire), `${actual.name} missing ${wire}`)
}

test('saved five-zone groups gain shared CDL unoccupied modes and retain edits', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks!.filter(block => !newRoot(block.name))
  old.outputs = old.outputs!.filter(port => !/^ahu_[12]_group_(setback|setup|freeze_setback|freeze_alarm_level)$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !newRoot(from.split('.')[0]) && !newRoot(to.split('.')[0]) &&
      !/\.group_(setback|setup|freeze_setback)$/.test(to) &&
      !/^ahu_[12]_group_(setback|setup|freeze_setback|freeze_alarm_level)$/.test(to) &&
      !/^zone_group_[12]\.scheduled_occupied$/.test(to)
  })
  for (const group of old.blocks.filter(block => /^zone_group_[12]$/.test(block.name))) {
    group.inputs = group.inputs!.filter(port => port.name !== 'scheduled_occupied' && !/^unoccupied_(heat|cool)_/.test(port.name))
    group.outputs = group.outputs!.filter(port => !['setback', 'setup', 'freeze_setback', 'freeze_alarm_level'].includes(port.name))
    group.blocks = group.blocks!.filter(block => !newGroupBlock(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !newGroupBlock(from.split('.')[0]) && !newGroupBlock(to.split('.')[0]) &&
        !/^unoccupied_(heat|cool)_/.test(from) && from !== 'scheduled_occupied' &&
        !['setback', 'setup', 'freeze_setback', 'freeze_alarm_level'].includes(to)
    })
    group.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 4200
  }
  for (const zone of old.blocks.filter(block => block.block_type === 'VAVBoxController')) {
    zone.inputs = zone.inputs!.filter(port => !['group_setback', 'group_setup', 'group_freeze_setback'].includes(port.name))
    zone.outputs = zone.outputs!.filter(port => !['unoccupied_heat_target', 'unoccupied_cool_target'].includes(port.name))
    zone.blocks = zone.blocks!.filter(block => !newZoneBlock(block.name))
    zone.connections = zone.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !newZoneBlock(from.split('.')[0]) && !newZoneBlock(to.split('.')[0]) &&
        !['group_setback', 'group_setup', 'group_freeze_setback'].includes(from) &&
        !['unoccupied_heat_target', 'unoccupied_cool_target'].includes(to)
    })
    zone.connections.push({ from: 'unoccupied_heat.y', to: 'heat_setpoint_active.u2' })
    zone.connections.push({ from: 'unoccupied_cool.y', to: 'cool_setpoint_active.u2' })
    zone.connections.push({ from: 'setback_needed.y', to: 'mode_select_setback.u2' })
    zone.connections.push({ from: 'setup_needed.y', to: 'mode_select_setup.u2' })
    zone.connections.push({ from: 'mode_select_setup.y', to: 'mode_select_cooldown.u3' })
    zone.notes = 'Saved zone note.'
  }
  old.blocks.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value = 19
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario), layout: { zone_group_1: { x: 24, y: 72 } } }
  const next = migrateG36GroupUnoccupied(saved, reference)
  assert.equal(next.graph.blocks!.filter(block => newRoot(block.name)).length, 20)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_group_1')!.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value, 4200)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!.find(block => block.name === 'unoccupied_heat')!.parameters![0].value, 19)
  assert.match(next.graph.blocks!.find(block => block.name === 'zone_01')!.notes!, /Saved zone note/)
  assert.deepEqual(next.layout.zone_group_1, { x: 24, y: 72 })
  assert.equal(next.graph.outputs!.filter(port => /^ahu_[12]_group_(setback|setup|freeze_setback|freeze_alarm_level)$/.test(port.name)).length, 8)
  assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === 'zone_01.group_setback').length, 1)
  expectTemplateWires(next.graph, reference)
  for (const name of ['zone_group_1', 'zone_group_2', 'zone_01', 'zone_10']) {
    expectTemplateWires(next.graph.blocks!.find(block => block.name === name)!, reference.blocks!.find(block => block.name === name)!)
  }
  assert.equal(migrateG36GroupUnoccupied(next, reference), next)
  const partly = structuredClone(saved)
  for (const group of partly.graph.blocks!.filter(block => /^zone_group_[12]$/.test(block.name))) {
    const current = reference.blocks!.find(block => block.name === group.name)!
    Object.assign(group, structuredClone(current))
  }
  const completed = migrateG36GroupUnoccupied(partly, reference)
  assert.equal(completed.graph.blocks!.filter(block => newRoot(block.name)).length, 20)
  assert.equal(completed.graph.blocks!.find(block => block.name === 'zone_group_1')!.inputs!.filter(port => port.name === 'unoccupied_heat_1').length, 1)
  expectTemplateWires(completed.graph, reference)
})
