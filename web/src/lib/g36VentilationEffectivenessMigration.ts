import { edgeEndpoints, type Block, type Project } from './types'

const oldName = 'ventilation_heating_supply'
const newName = 'ventilation_cooling_supply'

function hasStockSelector(zone: Block): boolean {
  if (zone.block_type !== 'VAVBoxController' || !zone.blocks?.some(block => block.name === oldName)) return false
  const edges = new Set((zone.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
  return [
    `discharge_temp→${oldName}.u1`, `room_temp→${oldName}.u2`,
    `ventilation_heating_effectiveness.y→ventilation_effectiveness.u1`,
    `ventilation_cooling_effectiveness.y→ventilation_effectiveness.u2`,
    `${oldName}.y→ventilation_effectiveness.u3`,
  ].every(edge => edges.has(edge)) && (zone.connections ?? [])
    .filter(edge => edgeEndpoints(edge).some(end => end.startsWith(`${oldName}.`))).length === 3
}

/** Use the pinned OBC cooling-supply comparator, retaining saved design rates and edited hysteresis. */
export function needsG36VentilationEffectivenessMigration(project: Project): boolean {
  if (project.graph.name !== 'FourPipeG36Controls'
    && !project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return (project.graph.blocks ?? []).some(hasStockSelector)
}

export function migrateG36VentilationEffectiveness(project: Project): Project {
  if (!needsG36VentilationEffectivenessMigration(project)) return project
  const next = structuredClone(project)
  for (const zone of next.graph.blocks ?? []) {
    if (!hasStockSelector(zone)) continue
    const comparator = zone.blocks!.find(block => block.name === oldName)!
    comparator.name = newName
    if (!comparator.notes || comparator.notes === 'A discharge warmer than the zone uses heating air-distribution effectiveness.') {
      comparator.notes = 'Measured room air warmer than discharge air selects cooling distribution effectiveness with OBC hysteresis.'
    }
    comparator.parameters ??= []
    if (!comparator.parameters.some(parameter => parameter.name === 'h')) {
      comparator.parameters.push({name: 'h', type: 'Real', value: 0.25, unit: 'K'})
    }
    for (const edge of zone.connections ?? []) {
      if (edge.from === `${oldName}.y`) edge.from = `${newName}.y`
      if (edge.to === `${oldName}.u1`) edge.to = `${newName}.u2`
      else if (edge.to === `${oldName}.u2`) edge.to = `${newName}.u1`
      if (edge.to === 'ventilation_effectiveness.u1'
        && edge.from === 'ventilation_heating_effectiveness.y') edge.from = 'ventilation_cooling_effectiveness.y'
      else if (edge.to === 'ventilation_effectiveness.u2'
        && edge.from === 'ventilation_cooling_effectiveness.y') edge.from = 'ventilation_heating_effectiveness.y'
    }
    const savedPosition = next.layout?.[`${zone.name}.${oldName}`]
    if (savedPosition) {
      next.layout[`${zone.name}.${newName}`] = savedPosition
      delete next.layout[`${zone.name}.${oldName}`]
    }
  }
  return next
}
