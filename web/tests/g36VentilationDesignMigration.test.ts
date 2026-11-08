import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36VentilationDesign } from '../src/lib/g36VentilationDesignMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved aggregate ventilation tuning becomes editable area and person rates', () => {
  const old = structuredClone(reference)
  old.inputs = old.inputs?.filter(port => !port.name.endsWith('_occupancy_count'))
  old.connections = old.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.endsWith('_occupancy_count') || end.endsWith('.occupancy_count')))
  if (old.brick_model) {
    old.brick_model.entities = old.brick_model.entities.filter(entity => !entity.id.endsWith('_occupancy_count'))
    old.brick_model.relationships = old.brick_model.relationships.filter(relationship => !relationship.object.endsWith('_occupancy_count'))
  }
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.inputs = zone.inputs?.filter(port => port.name !== 'occupancy_count')
    zone.blocks = zone.blocks?.filter(block => !['ventilation_floor_area', 'ventilation_area_rate', 'ventilation_person_rate', 'ventilation_window_eligible', 'co2_population_component'].includes(block.name))
    zone.connections = zone.connections?.filter(edge => !edgeEndpoints(edge).some(end => ['ventilation_floor_area.', 'ventilation_area_rate.', 'ventilation_person_rate.', 'ventilation_window_eligible.', 'co2_population_component.', 'occupancy_count'].some(stem => end.startsWith(stem))))
    zone.connections?.push(
      { from: 'ventilation_occupied.y', to: 'ventilation_area_active.u3' },
      { from: 'ventilation_occupied.y', to: 'ventilation_population_active.u3' },
      { from: 'ventilation_population_design.y', to: 'ventilation_population_active.u1' },
    )
    for (const [name, value] of [['ventilation_area_design', 0.04], ['ventilation_population_design', 0.06]] as const) {
      const block = zone.blocks?.find(child => child.name === name)!
      block.block_type = 'Constant'
      block.inputs = []
      block.parameters = [{ name: 'k', type: 'Real', value, unit: 'normalized airflow' }]
    }
  }
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario, design_occupants_per_zone: 10 }, layout: { zone_01: { x: 37, y: 91 } } }
  const migrated = migrateG36VentilationDesign(saved, reference)
  const zone = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  const rate = (name: string) => zone.blocks?.find(block => block.name === name)?.parameters?.[0].value
  assert.equal(rate('ventilation_area_rate'), 0.0004)
  assert.equal(rate('ventilation_person_rate'), 0.006)
  assert.equal(zone.blocks?.find(block => block.name === 'ventilation_area_design')?.block_type, 'Multiply')
  assert.ok(zone.inputs?.some(port => port.name === 'occupancy_count' && port.brick_class === 'Occupancy_Count_Sensor'))
  assert.ok(zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'occupancy_count→ventilation_population_design.u1'))
  assert.ok(zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'window_closed→ventilation_window_eligible.u2'))
  assert.ok(zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'co2_population_component.y→ventilation_population_active.u1'))
  assert.ok(!zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'ventilation_population_design.y→ventilation_population_active.u1'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'zone_01_occupancy_count→zone_01.occupancy_count'))
  assert.ok(migrated.graph.brick_model?.entities.some(entity => entity.id === 'zone_01_occupancy_count' && entity.class === 'brick:Occupancy_Count_Sensor'))
  assert.deepEqual(migrated.layout.zone_01, saved.layout.zone_01)
  assert.equal(migrateG36VentilationDesign(migrated, reference), migrated)
  assert.equal(saved.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'ventilation_area_design')?.block_type, 'Constant')
})
