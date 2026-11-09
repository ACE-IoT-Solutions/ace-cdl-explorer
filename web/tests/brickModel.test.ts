import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import jsonld from 'jsonld'
import { exportBrickModel, migrateBrickModel, validateBrickModel } from '../src/lib/brickModel'
import { physicalSignalsForGraph } from '../src/lib/signals'
import { defaultScenario, type Block, type Project } from '../src/lib/types'

const read = (name: string): Block => JSON.parse(readFileSync(new URL(`../public/examples/${name}`, import.meta.url), 'utf8')) as Block

for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  test(`${file} maps ten rooms to HVAC zones, VAVs, AHUs, and live connectors`, async () => {
    const graph = read(file)
    assert.deepEqual(validateBrickModel(graph), [])
    const model = graph.brick_model!
    const entities = new Map(model.entities.map(entity => [entity.id, entity]))
    assert.equal(model.entities.filter(entity => entity.class === 'rec:Room').length, 10)
    assert.equal(model.entities.filter(entity => entity.class === 'brick:HVAC_Zone').length, 10)
    for (let index = 1; index <= 10; index++) {
      const zone = `zone_${String(index).padStart(2, '0')}`
      const room = `space_${String(index).padStart(2, '0')}`
      const hvacZone = `hvac_${zone}`
      const vav = `vav_${String(index).padStart(2, '0')}_equipment`
      const ahu = `ahu_${Math.ceil(index / 5)}_equipment`
      assert.equal(entities.get(hvacZone)?.controller, zone)
      assert.equal(entities.get(`${zone}_room_temp`)?.connector, `${zone}_room_temp`)
      assert.equal(entities.get(`ahu_${Math.ceil(index / 5)}_actual_supply_air_temp`)?.class,
        'brick:Supply_Air_Temperature_Sensor')
      for (const [subject, predicate, object] of [
        [hvacZone, 'brick:hasPart', room], [ahu, 'brick:feeds', vav],
        [vav, 'brick:feeds', hvacZone], [hvacZone, 'brick:hasPoint', `${zone}_room_temp`],
        [ahu, 'brick:hasPoint', `ahu_${Math.ceil(index / 5)}_actual_supply_air_temp`],
      ]) assert.ok(model.relationships.some(relation => relation.subject === subject &&
        relation.predicate === predicate && relation.object === object))
      if (file === 'four_pipe_g36.json') {
        assert.equal(entities.get(`${zone}_discharge_temp`)?.class, 'brick:Discharge_Air_Temperature_Sensor')
        assert.ok(model.relationships.some(relation => relation.subject === vav &&
          relation.predicate === 'brick:hasPoint' && relation.object === `${zone}_discharge_temp`))
      }
    }
    const document = exportBrickModel(model)
    const rdf = await jsonld.toRDF(document) as unknown[]
    assert.ok(rdf.length > 100)
    assert.ok(physicalSignalsForGraph(graph).find(signal => signal.id === 'physical|zone_01_temp_c')
      ?.label.startsWith('Space 01 ·'))
  })
}

test('saved four-pipe projects gain spaces without replacing custom edits', () => {
  const reference = read('four_pipe_g36.json')
  const old = structuredClone(reference)
  delete old.brick_model
  old.blocks!.find(block => block.name === 'zone_01')!.notes = 'My custom terminal note'
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 42, y: 57 } } }
  const next = migrateBrickModel(saved, reference)
  assert.equal(next.graph.brick_model?.entities.filter(entity => entity.class === 'rec:Room').length, 10)
  assert.equal(next.graph.blocks!.find(block => block.name === 'zone_01')!.notes, 'My custom terminal note')
  assert.deepEqual(next.layout.zone_01, { x: 42, y: 57 })
  assert.equal(migrateBrickModel(next, reference), next)
  assert.equal(saved.graph.brick_model, undefined)
})

test('saved G36 spatial models gain discharge and AHU supply points without replacing edits', () => {
  const reference = read('four_pipe_g36.json')
  const old = structuredClone(reference)
  old.brick_model!.entities = old.brick_model!.entities.filter(entity =>
    entity.id !== 'zone_01_discharge_temp' && entity.id !== 'ahu_1_actual_supply_air_temp')
  old.brick_model!.relationships = old.brick_model!.relationships.filter(relation =>
    relation.object !== 'zone_01_discharge_temp' && relation.object !== 'ahu_1_actual_supply_air_temp')
  old.brick_model!.entities.find(entity => entity.id === 'space_01')!.label = 'Custom room'
  const saved: Project = { version: 1, name: old.name, graph: old,
    scenario: structuredClone(defaultScenario), layout: { zone_01: { x: 42, y: 57 } } }
  const next = migrateBrickModel(saved, reference)
  assert.notEqual(next, saved)
  assert.deepEqual(validateBrickModel(next.graph), [])
  assert.equal(next.graph.brick_model!.entities.find(entity => entity.id === 'space_01')!.label, 'Custom room')
  assert.equal(next.graph.brick_model!.entities.find(entity => entity.id === 'zone_01_discharge_temp')?.connector,
    'zone_01_discharge_temp')
  assert.equal(next.graph.brick_model!.entities.find(entity => entity.id === 'ahu_1_actual_supply_air_temp')?.connector,
    'ahu_1_actual_supply_air_temp')
  assert.deepEqual(next.layout.zone_01, { x: 42, y: 57 })
  assert.equal(migrateBrickModel(next, reference), next)
})

test('missing room mapping is reported before export', () => {
  const graph = read('four_pipe_building.json')
  graph.brick_model!.relationships = graph.brick_model!.relationships.filter(relation =>
    !(relation.subject === 'hvac_zone_01' && relation.predicate === 'brick:hasPart'))
  assert.ok(validateBrickModel(graph).some(error => error.includes('hvac_zone_01 must map to one modeled room')))
})
