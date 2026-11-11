import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36FreezeThresholds, needsG36FreezeThresholds } from '../src/lib/g36FreezeThresholdMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const names = ['freeze_stage_one_cold', 'freeze_stage_one_warm', 'freeze_stage_two_cold',
  'freeze_stage_three_below_fifteen', 'freeze_stage_three_below_five']

test('saved freeze comparators gain editable OBC hysteresis and retain tuned thresholds', () => {
  const graph = structuredClone(reference)
  graph.name = 'My G36 program'
  for (const ahu of graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    for (const name of names) {
      const comparator = ahu.blocks!.find(block => block.name === name)!
      const errorName = `${name}_error`
      const first = ahu.connections!.find(edge => edgeEndpoints(edge)[1] === `${errorName}.u1`)!
      const second = ahu.connections!.find(edge => edgeEndpoints(edge)[1] === `${errorName}.u2`)!
      ahu.blocks = ahu.blocks!.filter(block => block.name !== errorName)
      ahu.connections = ahu.connections!.filter(edge => !edgeEndpoints(edge)[1].startsWith(`${errorName}.`) &&
        edgeEndpoints(edge)[0] !== `${errorName}.y`)
      ahu.connections.push({ from: edgeEndpoints(first)[0], to: `${name}.u1` },
        { from: edgeEndpoints(second)[0], to: `${name}.u2` })
      comparator.block_type = name === 'freeze_stage_one_warm' ? 'Greater' : 'Less'
      comparator.inputs = [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }]
      comparator.parameters = []
    }
  }
  const ahuOne = graph.blocks!.find(block => block.name === 'ahu_1')!
  ahuOne.blocks!.find(block => block.name === 'freeze_stage_one_on')!.parameters![0].value = 4.0
  const saved: Project = { version: 1, name: graph.name, graph,
    scenario: structuredClone(defaultScenario), layout: { ahu_1: { x: 30, y: 42 } } }
  assert.equal(needsG36FreezeThresholds(saved), true)
  const migrated = migrateG36FreezeThresholds(saved, reference)
  assert.deepEqual(migrated.layout.ahu_1, saved.layout.ahu_1)
  for (const ahu of migrated.graph.blocks!.filter(block => block.block_type === 'AHUController')) {
    for (const name of names) {
      const comparator = ahu.blocks!.find(block => block.name === name)!
      assert.equal(comparator.block_type.split('.').at(-1), name === 'freeze_stage_one_warm' ? 'GreaterThreshold' : 'LessThreshold')
      assert.equal(comparator.parameters!.find(parameter => parameter.name === 'h')!.value, 0.25)
      assert.ok(ahu.connections!.some(edge => edgeEndpoints(edge).join('→') === `${name}_error.y→${name}.u`))
      assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}.u`).length, 1)
    }
  }
  assert.equal(migrated.graph.blocks!.find(block => block.name === 'ahu_1')!.blocks!
    .find(block => block.name === 'freeze_stage_one_on')!.parameters![0].value, 4.0)
  assert.equal(needsG36FreezeThresholds(migrated), false)
  assert.equal(migrateG36FreezeThresholds(migrated, reference), migrated)
})
