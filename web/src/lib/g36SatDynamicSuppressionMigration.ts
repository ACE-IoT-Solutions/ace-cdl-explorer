import { edgeEndpoints, type Block, type Project } from './types'

const replacementNames = new Set([
  'sat_change_edge', 'sat_change_setpoint', 'sat_change_room_temp',
  'sat_change_error', 'sat_change_error_reverse', 'sat_change_error_abs',
  'sat_suppression_rate', 'sat_suppression_maximum', 'sat_suppression_duration',
  'sat_suppression_timer_gate', 'sat_suppression_timer', 'sat_suppression_passed',
])
const oldNames = new Set(['sat_setpoint_stable', 'sat_suppression_elapsed'])
const eligibilityTargets = new Set(['sat_two_eligible.u2', 'sat_three_eligible.u2'])

export function needsG36SatDynamicSuppressionMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    (project.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController' &&
      zone.blocks?.some(child => child.name === 'sat_suppression_elapsed'))
}

/** Replace the fixed teaching delay; preserve previously tuned request thresholds. */
export function migrateG36SatDynamicSuppression(saved: Project, reference: Block): Project {
  if (!needsG36SatDynamicSuppressionMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController' || !zone.blocks?.some(child => child.name === 'sat_suppression_elapsed')) continue
    const template = reference.blocks?.find(child => child.name === zone.name)
    if (!template) throw new Error(`SAT suppression migration requires ${zone.name}`)
    zone.blocks = zone.blocks.filter(child => !oldNames.has(child.name))
    for (const child of template.blocks ?? []) {
      if (replacementNames.has(child.name) && !zone.blocks.some(existing => existing.name === child.name))
        zone.blocks.push(structuredClone(child))
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !oldNames.has(from.split('.')[0]) && !oldNames.has(to.split('.')[0]) && !eligibilityTargets.has(to)
    })
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!replacementNames.has(to.split('.')[0]) && !eligibilityTargets.has(to)) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { zone.connections.push(structuredClone(edge)); present.add(key) }
    }
    zone.notes = `${zone.notes ?? ''} SAT request suppression uses the captured zone error at a setpoint change, 540 s/K and a 30-minute cap.`.trim()
  }
  return next
}
