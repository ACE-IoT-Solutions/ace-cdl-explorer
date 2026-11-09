import { edgeEndpoints, type Block, type Project } from './types'

const names = new Set(['sat_cooling_loop_scale', 'sat_cooling_loop_limit'])

export function needsG36SatCoolingLoopMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    (project.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController' &&
      zone.blocks?.some(child => child.name === 'sat_cooling_loop_high') &&
      !zone.blocks?.some(child => child.name === 'sat_cooling_loop_scale'))
}

/** Feed SAT reset requests from the existing cooling PI, retaining request tuning. */
export function migrateG36SatCoolingLoop(saved: Project, reference: Block): Project {
  if (!needsG36SatCoolingLoopMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController' || !zone.blocks?.some(child => child.name === 'sat_cooling_loop_high') ||
        zone.blocks.some(child => child.name === 'sat_cooling_loop_scale')) continue
    const template = reference.blocks?.find(child => child.name === zone.name)
    if (!template) throw new Error(`Cooling-loop migration requires ${zone.name}`)
    for (const child of template.blocks ?? []) {
      if (names.has(child.name)) zone.blocks.push(structuredClone(child))
    }
    zone.connections = (zone.connections ?? []).filter(edge => edgeEndpoints(edge)[1] !== 'sat_cooling_loop_high.u')
    for (const edge of template.connections ?? []) {
      const target = edgeEndpoints(edge)[1]
      if (names.has(target.split('.')[0]) || target === 'sat_cooling_loop_high.u')
        zone.connections.push(structuredClone(edge))
    }
    zone.notes = `${zone.notes ?? ''} The SAT request uses the normalized cooling PI loop rather than a proportional error proxy.`.trim()
  }
  return next
}
