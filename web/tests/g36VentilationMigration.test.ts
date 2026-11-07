import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36Ventilation } from '../src/lib/g36VentilationMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block
const vent = (name: string) => name.startsWith('vent_') || name.includes('_vent_')

test('saved G36 graphs gain zone and AHU ventilation calculations without replacing PI tuning', () => {
  const old = structuredClone(reference)
  old.outputs = old.outputs?.filter(port => !port.name.includes('_ventilation_'))
  old.connections = old.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !from.includes('_ventilation_') && !to.includes('_ventilation_')
      && !from.includes('.ventilation_') && !to.includes('.ventilation_')
      && !(to.startsWith('ahu_') && to.endsWith('_measured_airflow'))
  })
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.outputs = zone.outputs?.filter(port => !port.name.startsWith('ventilation_'))
    zone.blocks = zone.blocks?.filter(block => !block.name.startsWith('ventilation_'))
    zone.connections = zone.connections?.filter(edge => !edgeEndpoints(edge).some(end => end.startsWith('ventilation_')))
  }
  for (const ahu of old.blocks?.filter(block => block.block_type === 'AHUController') ?? []) {
    ahu.inputs = ahu.inputs?.filter(port => !port.name.includes('_ventilation_') && !port.name.endsWith('_measured_airflow'))
    ahu.outputs = ahu.outputs?.filter(port => !port.name.startsWith('ventilation_'))
    ahu.blocks = ahu.blocks?.filter(block => !vent(block.name))
    ahu.connections = ahu.connections?.filter(edge => !edgeEndpoints(edge).some(end => vent(end.split('.')[0]) || end.startsWith('ventilation_') || end.endsWith('_measured_airflow')))
    ahu.blocks?.push({ name: 'minimum_oa_flow_gain', block_type: 'MultiplyByParameter', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 0.2 }] })
    ahu.connections?.push({ from: 'flow_request', to: 'minimum_oa_flow_gain.u' }, { from: 'minimum_oa_flow_gain.y', to: 'minimum_oa_setpoint.u1' })
  }
  const first = old.blocks?.find(block => block.name === 'ahu_1')!
  first.blocks!.find(block => block.name === 'minimum_oa_pi')!.parameters![0].value = 0.37
  const customZone = old.blocks?.find(block => block.name === 'zone_02')!
  customZone.blocks!.find(block => block.name === 'standby_airflow')!.parameters![0].value = 0.07
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { ahu_1: { x: 13, y: 27 } } }
  const migrated = migrateG36Ventilation(saved, reference)
  const ahu = migrated.graph.blocks?.find(block => block.name === 'ahu_1')!
  assert.equal(ahu.blocks?.find(block => block.name === 'minimum_oa_pi')?.parameters?.[0].value, 0.37)
  assert.ok(!ahu.blocks?.some(block => block.name === 'minimum_oa_flow_gain'))
  assert.ok(ahu.connections?.some(edge => edgeEndpoints(edge).join('→') === 'vent_effective_limit.y→minimum_oa_setpoint.u1'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'zone_01.ventilation_area→ahu_1.zone_01_ventilation_area'))
  assert.ok(migrated.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'zone_01_measured_airflow→ahu_1.zone_01_measured_airflow'))
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.find(block => block.name === 'standby_airflow')?.parameters?.[0].value, 0)
  assert.equal(migrated.graph.blocks?.find(block => block.name === 'zone_02')?.blocks?.find(block => block.name === 'standby_airflow')?.parameters?.[0].value, 0.07)
  assert.deepEqual(migrated.layout.ahu_1, { x: 13, y: 27 })
  assert.equal(migrateG36Ventilation(migrated, reference), migrated)
})
