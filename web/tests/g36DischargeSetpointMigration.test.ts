import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DischargeSetpoint, needsG36DischargeSetpointMigration } from '../src/lib/g36DischargeSetpointMigration'
import type { Block, Project } from '../src/lib/types'

const reference: Block = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8'))
const additions = new Set(['discharge_baseline', 'discharge_heat_detected', 'discharge_heat_hold'])

test('saved G36 VAVs gain the source discharge target while retaining valve tuning and layout', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks?.filter(block => !block.name.endsWith('_supply_target_previous'))
  old.connections = old.connections?.filter(edge => !edge.from.includes('_supply_target_previous.')
    && !edge.to.includes('_supply_target_previous.'))
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.inputs = zone.inputs?.filter(port => port.name !== 'supply_air_setpoint')
    zone.blocks = zone.blocks?.filter(block => !additions.has(block.name))
    zone.connections = zone.connections?.filter(edge => !additions.has(edge.from.split('.')[0])
      && !additions.has(edge.to.split('.')[0]))
    const oldDrivers: Record<string, string> = {
      'discharge_span.u2': 'supply_air_temp',
      'discharge_reset_target.u1': 'supply_air_temp',
      'discharge_target.u2': 'discharge_minimum.y',
      'discharge_target.u3': 'heating_mode.y',
      'discharge_valve_enable.u1': 'heating_mode.y',
    }
    for (const [target, source] of Object.entries(oldDrivers)) {
      const edge = zone.connections?.find(edge => edge.to === target)
      if (edge) edge.from = source
      else zone.connections?.push({from: source, to: target})
    }
  }
  const first = old.blocks?.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'discharge_max_rise')!.parameters!.find(parameter => parameter.name === 'k')!.value = 9
  first.blocks!.find(block => block.name === 'discharge_valve_pi')!.parameters!.find(parameter => parameter.name === 'k')!.value = 0.08
  first.notes = 'Custom zone note'
  const saved = {version: 1, name: old.name, graph: old, scenario: {} as Project['scenario'], layout: {zone_01: {x: 120, y: 340}}} as Project
  assert.equal(needsG36DischargeSetpointMigration(saved), true)
  const migrated = migrateG36DischargeSetpoint(saved, reference)
  const zone = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks?.find(block => block.name === 'discharge_max_rise')?.parameters?.find(parameter => parameter.name === 'k')?.value, 9)
  assert.equal(zone.blocks?.find(block => block.name === 'discharge_valve_pi')?.parameters?.find(parameter => parameter.name === 'k')?.value, 0.08)
  assert.equal(zone.notes, 'Custom zone note')
  assert.deepEqual(migrated.layout.zone_01, {x: 120, y: 340})
  assert.equal(zone.connections?.some(edge => edge.from === 'discharge_baseline.y' && edge.to === 'discharge_target.u2'), true)
  assert.equal(zone.connections?.some(edge => edge.from === 'discharge_heat_hold.y' && edge.to === 'discharge_target.u3'), true)
  assert.equal(migrated.graph.connections?.some(edge => edge.from === 'ahu_1_supply_target_previous.y' && edge.to === 'zone_01.supply_air_setpoint'), true)
  assert.equal(migrated.graph.connections?.some(edge => edge.from === 'ahu_2_supply_target_previous.y' && edge.to === 'zone_10.supply_air_setpoint'), true)
  assert.equal(needsG36DischargeSetpointMigration(migrated), false)
  assert.equal(migrateG36DischargeSetpoint(migrated, reference), migrated)
})
