import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateConventionalDamperCommands, needsConventionalDamperCommandsMigration } from '../src/lib/conventionalDamperCommandsMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block
const commands = ['outdoor_air_damper_position', 'return_air_damper_position']

test('saved conventional AHUs acquire CDL-driven damper pairs without losing outdoor-air tuning', () => {
  const graph = structuredClone(reference)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = graph.blocks!.find(block => block.name === name)!
    ahu.blocks = ahu.blocks!.filter(block => !block.name.startsWith('conventional_return_'))
    ahu.blocks!.find(block => block.name === 'minimum_outdoor_air')!.parameters![0].value = 0.4
    ahu.outputs = ahu.outputs!.filter(port => !commands.includes(port.name))
    ahu.connections = ahu.connections!.filter(edge =>
      !commands.includes(edgeEndpoints(edge)[1]) && !edgeEndpoints(edge)[1].startsWith('conventional_return_'))
    graph.outputs = graph.outputs!.filter(port => !commands.some(command => port.name === `${name}_${command}`))
    graph.connections = graph.connections!.filter(edge =>
      !commands.some(command => edgeEndpoints(edge)[1] === `${name}_${command}`))
  }
  const saved: Project = { version: 1, name: graph.name, graph, scenario: structuredClone(defaultScenario),
    layout: { ahu_1: { x: 12, y: 34 } } }
  assert.ok(needsConventionalDamperCommandsMigration(saved))
  const next = migrateConventionalDamperCommands(saved, reference)
  assert.deepEqual(next.layout.ahu_1, { x: 12, y: 34 })
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks!.find(block => block.name === name)!
    assert.equal(ahu.blocks!.find(block => block.name === 'minimum_outdoor_air')!.parameters![0].value, 0.4)
    assert.ok(ahu.blocks!.some(block => block.name === 'conventional_return_complement'))
    for (const command of commands) {
      assert.equal(ahu.outputs!.filter(port => port.name === command).length, 1)
      assert.equal(ahu.connections!.filter(edge => edgeEndpoints(edge)[1] === command).length, 1)
      assert.equal(next.graph.outputs!.filter(port => port.name === `${name}_${command}`).length, 1)
      assert.equal(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === `${name}_${command}`).length, 1)
    }
  }
  assert.equal(needsConventionalDamperCommandsMigration(next), false)
  assert.equal(migrateConventionalDamperCommands(next, reference), next)
})
