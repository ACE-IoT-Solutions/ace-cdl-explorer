import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36CoolingStartup } from '../src/lib/g36CoolingStartupMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved VAV cooling requests no longer depend on warm AHU supply during startup', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.blocks?.push(
      { name: 'air_can_cool', block_type: 'Less', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Boolean' }] },
      { name: 'cooling_enabled', block_type: 'And', inputs: [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }] },
    )
    zone.connections = zone.connections?.filter(edge => edgeEndpoints(edge).join('→') !== 'cooling_mode.y→damper_select.u3')
    zone.connections?.push(
      { from: 'supply_air_temp', to: 'air_can_cool.u1' },
      { from: 'room_temp', to: 'air_can_cool.u2' },
      { from: 'cooling_mode.y', to: 'cooling_enabled.u1' },
      { from: 'air_can_cool.y', to: 'cooling_enabled.u2' },
      { from: 'cooling_enabled.y', to: 'damper_select.u3' },
    )
  }
  const first = old.blocks?.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'cool_setpoint')!.parameters![0].value = 23.1
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { zone_01: { x: 11, y: 21 } } }
  const migrated = migrateG36CoolingStartup(saved)
  const zone = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  assert.equal(zone.blocks?.find(block => block.name === 'cool_setpoint')?.parameters?.[0].value, 23.1)
  assert.ok(!zone.blocks?.some(block => block.name === 'cooling_enabled'))
  assert.ok(zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'cooling_mode.y→damper_select.u3'))
  assert.equal(zone.connections?.filter(edge => edgeEndpoints(edge)[1] === 'damper_select.u3').length, 1)
  assert.deepEqual(migrated.layout.zone_01, { x: 11, y: 21 })
  assert.equal(migrateG36CoolingStartup(migrated), migrated)
})
