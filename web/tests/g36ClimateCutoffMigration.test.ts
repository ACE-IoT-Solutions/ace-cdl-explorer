import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { climateCutoffBlockNames, migrateG36ClimateCutoff, needsG36ClimateCutoff } from '../src/lib/g36ClimateCutoffMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(climateCutoffBlockNames)

test('saved renamed G36 AHUs gain a climate cutoff without replacing manually tuned fixed limits', () => {
  const graph = structuredClone(reference)
  graph.name = 'MyG36Building'
  for (const ahu of graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'economizer_fixed_dry_bulb_cutoff.y', to: 'economizer_fixed_dry_bulb_error.u2' })
  }
  const first = graph.blocks!.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value = 22
  first.notes = 'My AHU notes'
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 28, y: 39 } } }
  assert.equal(needsG36ClimateCutoff(saved), true)
  const migrated = migrateG36ClimateCutoff(saved, reference)
  assert.deepEqual(migrated.layout.ahu_1, { x: 28, y: 39 })
  for (const index of [1, 2]) {
    const ahu = migrated.graph.blocks!.find(block => block.name === `ahu_${index}`)!
    for (const name of climateCutoffBlockNames) assert.equal(ahu.blocks!.filter(block => block.name === name).length, 1, name)
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'economizer_fixed_dry_bulb_error.u2').length, 1)
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'economizer_effective_fixed_cutoff.y→economizer_fixed_dry_bulb_error.u2'))
    assert.equal(ahu.blocks!.find(block => block.name === 'economizer_use_climate_cutoff')!.parameters![0].value, index === 1 ? false : true)
  }
  const migratedFirst = migrated.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.equal(migratedFirst.blocks!.find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')!.parameters![0].value, 22)
  assert.equal(migratedFirst.notes, 'My AHU notes')
  assert.equal(needsG36ClimateCutoff(migrated), false)
  assert.equal(migrateG36ClimateCutoff(migrated, reference), migrated)

  const rewired = structuredClone(saved)
  const customAhu = rewired.graph.blocks!.find(block => block.name === 'ahu_1')!
  customAhu.connections!.find(edge => edgeEndpoints(edge)[1] === 'economizer_fixed_dry_bulb_error.u2')!.from = 'sat_mode_warm_target.y'
  const custom = migrateG36ClimateCutoff(rewired, reference)
  const migratedCustomAhu = custom.graph.blocks!.find(block => block.name === 'ahu_1')!
  assert.ok(migratedCustomAhu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
    'sat_mode_warm_target.y→economizer_effective_fixed_cutoff.u2'))
  assert.equal(migratedCustomAhu.blocks!.find(block => block.name === 'economizer_use_climate_cutoff')!.parameters![0].value, false)
})
