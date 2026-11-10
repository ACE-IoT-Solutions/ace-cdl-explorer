import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OptionalDemandGate, needsG36OptionalDemandGateMigration } from '../src/lib/g36OptionalDemandGateMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = new Set(['economizer_use_teaching_demand_gate', 'economizer_demand_gate_bypass',
  'economizer_demand_gate_select'])

test('new G36 samples bypass the extra demand gate while saved controls retain it', () => {
  const old = structuredClone(reference)
  for (const ahu of old.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.equal(ahu.blocks!.find(block => block.name === 'economizer_use_teaching_demand_gate')!.parameters![0].value, false)
    ahu.blocks = ahu.blocks!.filter(block => !added.has(block.name))
    ahu.connections = ahu.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added.has(from.split('.')[0]) && !added.has(to.split('.')[0])
    })
    ahu.connections.push({ from: 'return_above_supply_target.y', to: 'free_cooling_useful.u2' })
  }
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 37, y: 21 } } }
  assert.ok(needsG36OptionalDemandGateMigration(saved))
  const renamed = structuredClone(saved)
  renamed.name = 'My building controls'
  renamed.graph.name = renamed.name
  assert.ok(needsG36OptionalDemandGateMigration(renamed))
  assert.equal(migrateG36OptionalDemandGate(renamed, reference).graph.name, renamed.name)
  const next = migrateG36OptionalDemandGate(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 37, y: 21 })
  for (const ahu of next.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    assert.equal(ahu.blocks!.find(block => block.name === 'economizer_use_teaching_demand_gate')!.parameters![0].value, true)
    assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') ===
      'return_above_supply_target.y→economizer_demand_gate_select.u1'))
    assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === 'free_cooling_useful.u2').length, 1)
  }
  assert.equal(needsG36OptionalDemandGateMigration(next), false)
  assert.equal(migrateG36OptionalDemandGate(next, reference), next)
  const custom = structuredClone(saved)
  custom.graph.blocks!.find(block => block.name === 'ahu_1')!.connections!
    .find(edge => edgeEndpoints(edge)[1] === 'free_cooling_useful.u2')!.from = 'economizer_freeze_clear.y'
  assert.ok(migrateG36OptionalDemandGate(custom, reference).graph.blocks!
    .find(block => block.name === 'ahu_1')!.connections!
    .some(edge => edgeEndpoints(edge).join('→') === 'economizer_freeze_clear.y→economizer_demand_gate_select.u1'))
})
