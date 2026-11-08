import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36OperationMode, needsG36OperationModeMigration } from '../src/lib/g36OperationModeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const added = (name: string): boolean => name === 'obc_operation_mode' || name.startsWith('mode_')
const oldSources: Record<string, string> = {
  warmup: 'warmup_selected.y', cooldown: 'cooldown_selected.y',
  setback: 'setback_active.y', setup: 'setup_active.y', freeze_setback: 'freeze_active.y',
  'any_preconditioning.u1': 'warmup_selected.y',
  'any_preconditioning.u2': 'cooldown_selected.y',
  'optimal_warmup_active.u': 'warmup_selected.y',
  'optimal_cooldown_active.u': 'cooldown_selected.y',
  'freeze_alarm_select.u2': 'freeze_active.y',
}

test('saved G36 groups gain OBC operation mode while preserving tuned cap and freeze thresholds', () => {
  const saved: Project = {
    version: 1, name: reference.name, graph: structuredClone(reference),
    scenario: structuredClone(defaultScenario), layout: {},
  }
  for (const index of [1, 2]) {
    const group = saved.graph.blocks!.find(block => block.name === `zone_group_${index}`)!
    group.blocks = group.blocks!.filter(block => !added(block.name))
    group.connections = group.connections!.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !added(from.split('.')[0]) && !added(to.split('.')[0]) && to !== 'operation_mode'
    })
    group.outputs = group.outputs!.filter(port => port.name !== 'operation_mode')
    for (const [to, from] of Object.entries(oldSources))
      group.connections.push({ from, to })
    group.blocks.find(block => block.name === 'maximum_start_seconds')!.parameters![0].value = 7200
    group.blocks.find(block => block.name === 'freeze_setback_on')!.parameters![0].value = 3
    group.blocks.find(block => block.name === 'freeze_setback_off')!.parameters![0].value = 8
  }
  saved.graph.outputs = saved.graph.outputs!.filter(port => !port.name.endsWith('_group_operation_mode'))
  saved.graph.connections = saved.graph.connections!.filter(edge => !edgeEndpoints(edge)[1].endsWith('_group_operation_mode'))
  assert.equal(needsG36OperationModeMigration(saved), true)
  const upgraded = migrateG36OperationMode(saved, reference)
  assert.equal(needsG36OperationModeMigration(upgraded), false)
  for (const index of [1, 2]) {
    const group = upgraded.graph.blocks!.find(block => block.name === `zone_group_${index}`)!
    const template = reference.blocks!.find(block => block.name === `zone_group_${index}`)!
    assert.equal(group.blocks!.length, template.blocks!.length)
    const mode = group.blocks!.find(block => block.name === 'obc_operation_mode')!
    const parameter = (child: string, name: string) => mode.blocks!.find(block => block.name === child)!
      .parameters!.find(parameter => parameter.name === name)!.value
    assert.equal(parameter('maxWarCooTime', 'k'), 7200)
    assert.equal(parameter('truFalHol', 'trueHoldDuration'), 7200)
    assert.equal(parameter('truFalHol1', 'trueHoldDuration'), 7200)
    assert.equal(parameter('actFreProTem', 'k'), 3)
    assert.equal(parameter('endFreProTem', 'k'), 8)
    const edges = (block: Block) => block.connections!.map(edge => edgeEndpoints(edge).join('→')).sort()
    assert.deepEqual(edges(group), edges(template))
    assert.ok(upgraded.graph.outputs!.some(port => port.name === `ahu_${index}_group_operation_mode`))
  }
  assert.equal(migrateG36OperationMode(upgraded, reference), upgraded)
})
