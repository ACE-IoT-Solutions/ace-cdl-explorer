import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36PollutionMode } from '../src/lib/g36PollutionModeMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 graph gains CDL pollution mode without replacing tuned AHU controls', () => {
  const old = structuredClone(reference)
  old.blocks = old.blocks?.filter(block => block.name !== 'outdoor_air_pollution_mode')
  old.inputs = old.inputs?.filter(port => !port.name.startsWith('pollution_'))
  old.outputs = old.outputs?.filter(port => !port.name.startsWith('pollution_'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
    end.startsWith('pollution_') || end.startsWith('outdoor_air_pollution_mode.')))
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.inputs = ahu.inputs?.filter(port => port.name !== 'pollution_active')
    ahu.blocks = ahu.blocks?.filter(block => !['pollution_clear', 'economizer_pollution_gate'].includes(block.name))
    ahu.connections = ahu.connections?.filter(edge => !edgeEndpoints(edge).some(end =>
      end.startsWith('pollution_') || end.startsWith('economizer_pollution_gate.') || end.startsWith('pollution_clear.')))
    ahu.connections?.push({from:'economizer_temperature_ok.y',to:'economizer_enabled.u2'})
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'economizer_mixed_air_pi')!.parameters![0].value = 0.23
  const saved: Project = {version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{ahu_1:{x:19,y:31}}}
  const migrated = migrateG36PollutionMode(saved, reference)
  assert.ok(migrated.graph.blocks?.some(block => block.name === 'outdoor_air_pollution_mode'))
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks
    ?.find(block => block.name === 'economizer_mixed_air_pi')?.parameters?.[0].value, 0.23)
  for (const ahu of migrated.graph.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    assert.deepEqual(ahu.connections?.filter(edge => edgeEndpoints(edge)[1] === 'economizer_enabled.u2')
      .map(edge => edgeEndpoints(edge)[0]), ['economizer_pollution_gate.y'])
  }
  assert.deepEqual(migrated.layout.ahu_1,{x:19,y:31})
  assert.equal(migrateG36PollutionMode(migrated,reference),migrated)
})
