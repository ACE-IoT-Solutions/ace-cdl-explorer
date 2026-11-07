import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36CommonDamperMapping } from '../src/lib/g36CommonDamperMappingMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved common-damper graphs gain PI range mapping without losing tuning', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.blocks = ahu.blocks?.filter(block => !['minimum_oa_lower_half', 'minimum_oa_outdoor_stroke'].includes(block.name))
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !['minimum_oa_lower_half', 'minimum_oa_outdoor_stroke'].includes(from.split('.')[0])
        && !['minimum_oa_lower_half', 'minimum_oa_outdoor_stroke'].includes(to.split('.')[0])
        && edgeEndpoints(edge).join('→') !== 'outdoor_air_select.y→return_damper_complement.u2'
    })
    ahu.connections?.push(
      { from: 'minimum_oa_pi.y', to: 'minimum_oa_active.u1' },
      { from: 'outdoor_damper_physical_limit.y', to: 'return_damper_complement.u2' },
    )
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'minimum_oa_pi')!.parameters![0].value = 0.37
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 13, y: 27 } } }
  const migrated = migrateG36CommonDamperMapping(saved, reference)
  const ahu = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  assert.equal(ahu.blocks?.find(block => block.name === 'minimum_oa_pi')?.parameters?.[0].value, 0.37)
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'minimum_oa_outdoor_stroke.y→minimum_oa_active.u1'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'outdoor_air_select.y→return_damper_complement.u2'))
  assert.ok(!ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'outdoor_damper_physical_limit.y→return_damper_complement.u2'))
  assert.deepEqual(migrated.layout.ahu_1, { x: 13, y: 27 })
  assert.equal(migrateG36CommonDamperMapping(migrated, reference), migrated)
})
