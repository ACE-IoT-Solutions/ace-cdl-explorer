import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36TimedOverrides, needsG36TimedOverrideMigration } from '../src/lib/g36TimedOverrideMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved raw override contacts gain editable timers without replacing zone tuning', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks!.filter(block => !/^zone_\d{2}_override_(edge|hold)$/.test(block.name))
  old.outputs = old.outputs!.filter(port => !/^zone_\d{2}_override_active$/.test(port.name))
  old.connections = old.connections!.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !/^zone_\d{2}_override_(edge|hold)(\.|$)/.test(from) &&
      !/^zone_\d{2}_override_(edge|hold)(\.|$)/.test(to) &&
      !/^zone_\d{2}_override_active$/.test(to)
  })
  for (let zone = 1; zone <= 10; zone++) {
    const name = `zone_${String(zone).padStart(2, '0')}`
    const group = `zone_group_${Math.ceil(zone / 5)}`
    const local = (zone - 1) % 5 + 1
    old.connections.push({ from: `${name}_override`, to: `${group}.override_${local}` },
      { from: `${name}_override`, to: `${name}.local_override` })
  }
  old.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value = 22
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: structuredClone(defaultScenario),
    layout: { zone_01: { x: 17, y: 23 } } }
  assert.ok(needsG36TimedOverrideMigration(saved))
  const next = migrateG36TimedOverrides(saved, reference)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.blocks!
    .find(block => block.name === 'heat_setpoint')!.parameters![0].value, 22)
  assert.deepEqual(next.layout.zone_01, { x: 17, y: 23 })
  for (let zone = 1; zone <= 10; zone++) {
    const name = `zone_${String(zone).padStart(2, '0')}`
    const hold = next.graph.blocks!.find(block => block.name === `${name}_override_hold`)!
    assert.equal(hold.parameters!.find(parameter => parameter.name === 'trueHoldDuration')!.value, 7200)
    for (const target of [`zone_group_${Math.ceil(zone / 5)}.override_${(zone - 1) % 5 + 1}`, `${name}.local_override`]) {
      assert.deepEqual(next.graph.connections!.filter(edge => edgeEndpoints(edge)[1] === target)
        .map(edge => edgeEndpoints(edge)[0]), [`${name}_override_hold.y`])
    }
  }
  assert.equal(needsG36TimedOverrideMigration(next), false)
  assert.equal(migrateG36TimedOverrides(next, reference), next)
})
