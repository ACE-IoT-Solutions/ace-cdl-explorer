import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36ZoneStatus, needsG36ZoneStatusMigration } from '../src/lib/g36ZoneStatusMigration'
import { defaultScenario, edgeEndpoints, type Block, type Connection, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const legacy = JSON.parse(readFileSync(new URL('./fixtures/g36_pre_zone_status_group.json', import.meta.url), 'utf8')) as {
  blocks: Block[]; connections: Connection[]
}
const added = (name: string): boolean => /^zone_status_[1-5]$/.test(name) ||
  /^(warmup|cooldown)_status_(gate|any_[2-5])$/.test(name)

test('saved G36 groups gain OBC zone status without replacing edited recovery tuning', () => {
  const saved: Project = {
    version: 1, name: reference.name, graph: structuredClone(reference),
    scenario: structuredClone(defaultScenario), layout: {},
  }
  for (const index of [1, 2]) {
    const group = saved.graph.blocks!.find(block => block.name === `zone_group_${index}`)!
    group.blocks = group.blocks!.filter(block => !added(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added(from.split('.')[0]) && !added(to.split('.')[0])
    })
    group.blocks.push(...structuredClone(legacy.blocks))
    group.connections.push(...structuredClone(legacy.connections))
    for (const mode of ['warmup', 'cooldown']) {
      group.connections.push({ from: `${mode}_start.y`, to: `${mode}_candidate.u1` })
      for (let zone = 1; zone <= 5; zone++) {
        const old = `zone_status_${zone}.${mode === 'warmup' ? 'yWarTim' : 'yCooTim'}`
        const template = reference.blocks!.find(block => block.name === `zone_group_${index}`)!
        for (const edge of template.connections!) {
          const [from, to] = edgeEndpoints(edge)
          if (from === old && !added(to.split('.')[0]))
            group.connections.push({ from: `optimal_${mode}_${zone}_predictor.seconds`, to })
        }
      }
    }
    group.blocks.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value = 1800
    group.blocks.find(block => block.name === 'setback_end_margin')!.parameters![0].value = 1.5
    group.blocks.find(block => block.name === 'setup_end_margin')!.parameters![0].value = 2
  }
  assert.equal(needsG36ZoneStatusMigration(saved), true)
  const upgraded = migrateG36ZoneStatus(saved, reference)
  assert.equal(needsG36ZoneStatusMigration(upgraded), false)
  for (const index of [1, 2]) {
    const group = upgraded.graph.blocks!.find(block => block.name === `zone_group_${index}`)!
    const template = reference.blocks!.find(block => block.name === `zone_group_${index}`)!
    assert.equal(group.blocks!.filter(block => block.block_type === 'ZoneStatus').length, 5)
    assert.equal(group.blocks!.length, template.blocks!.length)
    assert.equal(group.blocks!.find(block => block.name === 'warmup_seconds_per_k')!.parameters![0].value, 1800)
    for (let zone = 1; zone <= 5; zone++) {
      const status = group.blocks!.find(block => block.name === `zone_status_${zone}`)!
      assert.equal(status.blocks!.find(block => block.name === 'hys3')!.parameters!.find(p => p.name === 'uHigh')!.value, 1.5)
      assert.equal(status.blocks!.find(block => block.name === 'hys4')!.parameters!.find(p => p.name === 'uHigh')!.value, 2)
    }
    const edges = (block: Block) => block.connections!.map(edge => edgeEndpoints(edge).join('→')).sort()
    assert.deepEqual(edges(group), edges(template))
  }
  assert.equal(migrateG36ZoneStatus(upgraded, reference), upgraded)
})
