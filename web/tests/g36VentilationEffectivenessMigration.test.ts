import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36VentilationEffectiveness, needsG36VentilationEffectivenessMigration } from '../src/lib/g36VentilationEffectivenessMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved zone ventilation selectors adopt OBC cooling hysteresis without losing tuning', () => {
  const old = structuredClone(reference)
  old.name = 'Renamed G36 experiment'
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    const comparator = zone.blocks!.find(block => block.name === 'ventilation_cooling_supply')!
    comparator.name = 'ventilation_heating_supply'
    comparator.parameters = []
    for (const edge of zone.connections ?? []) {
      if (edge.from === 'ventilation_cooling_supply.y') edge.from = 'ventilation_heating_supply.y'
      if (edge.to === 'ventilation_cooling_supply.u1') edge.to = 'ventilation_heating_supply.u2'
      else if (edge.to === 'ventilation_cooling_supply.u2') edge.to = 'ventilation_heating_supply.u1'
      if (edge.to === 'ventilation_effectiveness.u1' && edge.from === 'ventilation_cooling_effectiveness.y') edge.from = 'ventilation_heating_effectiveness.y'
      else if (edge.to === 'ventilation_effectiveness.u2' && edge.from === 'ventilation_heating_effectiveness.y') edge.from = 'ventilation_cooling_effectiveness.y'
    }
  }
  const first = old.blocks!.find(block => block.name === 'zone_01')!
  first.blocks!.find(block => block.name === 'ventilation_heating_supply')!.parameters = [{name:'h',value:0.4,unit:'K'}]
  first.blocks!.find(block => block.name === 'ventilation_heating_supply')!.notes = 'My sensor transition note'
  first.blocks!.find(block => block.name === 'ventilation_floor_area')!.parameters![0].value = 160
  const project: Project = {version:1,name:old.name,graph:old,scenario:structuredClone(defaultScenario),layout:{'zone_01.ventilation_heating_supply':{x:42,y:63}}}
  assert.equal(needsG36VentilationEffectivenessMigration(project),true)
  const migrated = migrateG36VentilationEffectiveness(project)
  assert.equal(migrated.graph.blocks?.filter(block => block.block_type === 'VAVBoxController').length,10)
  for (const zone of migrated.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    assert.equal(zone.blocks?.find(block => block.name === 'ventilation_cooling_supply')?.parameters?.find(parameter => parameter.name === 'h')?.value,zone.name === 'zone_01' ? 0.4 : 0.25)
    const edges = new Set(zone.connections?.map(edge => edgeEndpoints(edge).join('→')))
    assert.ok(edges.has('room_temp→ventilation_cooling_supply.u1'))
    assert.ok(edges.has('discharge_temp→ventilation_cooling_supply.u2'))
    assert.ok(edges.has('ventilation_cooling_effectiveness.y→ventilation_effectiveness.u1'))
    assert.ok(edges.has('ventilation_heating_effectiveness.y→ventilation_effectiveness.u2'))
  }
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'ventilation_floor_area')?.parameters?.[0].value,160)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'ventilation_cooling_supply')?.notes,'My sensor transition note')
  assert.deepEqual(migrated.layout['zone_01.ventilation_cooling_supply'],{x:42,y:63})
  assert.equal(migrated.layout['zone_01.ventilation_heating_supply'],undefined)
  assert.equal(migrateG36VentilationEffectiveness(migrated),migrated)
  assert.equal(project.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.some(block => block.name === 'ventilation_heating_supply'),true)
})

test('custom ventilation wiring is left intact', () => {
  const graph=structuredClone(reference)
  const zone=graph.blocks!.find(block=>block.name==='zone_01')!
  zone.blocks!.find(block=>block.name==='ventilation_cooling_supply')!.name='ventilation_heating_supply'
  const project:Project={version:1,name:graph.name,graph,scenario:structuredClone(defaultScenario),layout:{}}
  assert.equal(needsG36VentilationEffectivenessMigration(project),false)
  assert.equal(migrateG36VentilationEffectiveness(project),project)
})
