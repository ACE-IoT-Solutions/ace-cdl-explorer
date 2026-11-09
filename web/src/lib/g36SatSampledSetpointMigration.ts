import { edgeEndpoints, type Block, type Project } from './types'

const rewiredTargets = new Set([
  'sat_previous_cooling_setpoint.u', 'sat_setpoint_rise.u1', 'sat_setpoint_fall.u2',
])

export function needsG36SatSampledSetpointMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    (project.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController' &&
      zone.blocks?.some(child => child.name === 'sat_previous_cooling_setpoint') &&
      !zone.blocks?.some(child => child.name === 'sat_sampled_cooling_setpoint'))
}

/** Sample the setpoint before comparing it with its previous sampled value. */
export function migrateG36SatSampledSetpoint(saved: Project, reference: Block): Project {
  if (!needsG36SatSampledSetpointMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController' || !zone.blocks?.some(child => child.name === 'sat_previous_cooling_setpoint') ||
        zone.blocks.some(child => child.name === 'sat_sampled_cooling_setpoint')) continue
    const template = reference.blocks?.find(child => child.name === zone.name)
    const sampler = template?.blocks?.find(child => child.name === 'sat_sampled_cooling_setpoint')
    if (!sampler) throw new Error(`SAT setpoint sampling migration requires ${zone.name}`)
    zone.blocks.push(structuredClone(sampler))
    zone.connections = (zone.connections ?? []).filter(edge => !rewiredTargets.has(edgeEndpoints(edge)[1]))
    for (const edge of template!.connections ?? []) {
      const target = edgeEndpoints(edge)[1]
      if (target === 'sat_sampled_cooling_setpoint.u' || rewiredTargets.has(target))
        zone.connections.push(structuredClone(edge))
    }
    zone.notes = `${zone.notes ?? ''} Setpoint-change suppression compares sampled, not live, cooling targets.`.trim()
  }
  return next
}
