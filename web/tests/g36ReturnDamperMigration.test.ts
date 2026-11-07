import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ReturnDamper } from '../src/lib/g36ReturnDamperMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set([
  'return_damper_one', 'return_damper_complement', 'return_damper_pi_midpoint',
  'return_damper_pi_excess', 'return_damper_zero', 'return_damper_positive_excess',
  'return_damper_double_excess', 'return_damper_pi_max', 'return_damper_oa_limited',
  'return_damper_mode', 'return_damper_physical_max', 'return_damper_physical_limit',
])

test('saved G36 AHUs gain independent return damper commands and retain tuned controls', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs?.filter(port => !port.name.endsWith('_air_damper_position'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge)[1].endsWith('_air_damper_position'))
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.outputs = ahu.outputs?.filter(port => !port.name.endsWith('_air_damper_position'))
    ahu.blocks = ahu.blocks?.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
        && !to.endsWith('_air_damper_position')
    })
  }
  const ahu = old.blocks?.find(block => block.name === 'ahu_1')!
  ahu.blocks!.find(block => block.name === 'minimum_oa_pi')!.parameters![0].value = 0.37
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 13, y: 27 } } }
  const migrated = migrateG36ReturnDamper(saved, reference)
  const first = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  assert.equal(first.blocks?.find(block => block.name === 'minimum_oa_pi')?.parameters?.[0].value, 0.37)
  assert.ok(first.blocks?.some(block => block.name === 'return_damper_physical_limit'))
  assert.ok(first.connections?.some(edge => edgeEndpoints(edge).join('→') === 'return_damper_physical_limit.y→return_air_damper_position'))
  assert.ok(migrated.graph.outputs?.some(port => port.name === 'ahu_2_return_air_damper_position'))
  assert.deepEqual(migrated.layout.ahu_1, { x: 13, y: 27 })
  assert.equal(migrateG36ReturnDamper(migrated, reference), migrated)
})
