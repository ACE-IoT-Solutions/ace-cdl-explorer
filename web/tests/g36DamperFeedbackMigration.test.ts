import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateG36DamperFeedback } from '../src/lib/g36DamperFeedbackMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json', import.meta.url), 'utf8')) as Block

test('saved G36 zones gain measured-airflow damper control without losing tuning', () => {
  const old = structuredClone(reference)
  for (const zone of old.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []) {
    zone.blocks = zone.blocks?.filter(block => !['damper_flow_pi', 'damper_flow_threshold', 'damper_flow_requested', 'damper_closed', 'damper_feedback_output', 'measured_airflow_passthrough'].includes(block.name))
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !from.startsWith('damper_flow_') && !to.startsWith('damper_flow_')
        && from !== 'damper_closed.y' && to !== 'damper_feedback_output.u1'
        && from !== 'damper_feedback_output.y' && !from.startsWith('measured_airflow_passthrough.') && !to.startsWith('measured_airflow_passthrough.')
    })
    zone.connections?.push(
      { from: 'heating_damper_select.y', to: 'damper_position' },
      { from: 'heating_damper_select.y', to: 'damper_nearly_open.u1' },
      { from: 'airflow_sum.y', to: 'airflow' },
    )
  }
  const zone = old.blocks?.find(block => block.name === 'zone_01')!
  zone.notes = 'Keep my note'
  zone.blocks!.find(block => block.name === 'heating_damper_gain')!.parameters![0].value = 0.6
  const saved: Project = { version: 1, name: old.name, graph: old, scenario: { ...defaultScenario }, layout: { zone_01: { x: 42, y: 57 } } }
  const migrated = migrateG36DamperFeedback(saved, reference)
  const updated = migrated.graph.blocks?.find(block => block.name === 'zone_01')!
  assert.equal(updated.notes, 'Keep my note')
  assert.equal(updated.blocks?.find(block => block.name === 'heating_damper_gain')?.parameters?.[0].value, 0.6)
  assert.ok(updated.blocks?.some(block => block.name === 'damper_flow_pi'))
  assert.ok(updated.connections?.some(edge => edgeEndpoints(edge).join('→') === 'damper_feedback_output.y→damper_position'))
  assert.ok(updated.connections?.some(edge => edgeEndpoints(edge).join('→') === 'measured_airflow_passthrough.y→airflow'))
  assert.deepEqual(migrated.layout.zone_01, { x: 42, y: 57 })
  assert.equal(migrateG36DamperFeedback(migrated, reference), migrated)
})
