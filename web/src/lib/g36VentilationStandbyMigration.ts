import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'ventilation_standby_permitted',
  'ventilation_standby_disabled',
  'ventilation_area_presence_or_required',
  'ventilation_area_mode_eligible',
  'ventilation_area_window_eligible',
])

export function needsG36VentilationStandbyMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    (project.graph.blocks ?? []).filter(block => block.block_type === 'VAVBoxController')
      .some(zone => zone.blocks?.some(child => child.name === 'window_primary_minimum') &&
        !zone.blocks?.some(child => child.name === 'ventilation_standby_permitted'))
}

/** Make the occupied-standby permission a saved, editable CDL choice. */
export function migrateG36VentilationStandby(project: Project, reference: Block): Project {
  if (!needsG36VentilationStandbyMigration(project)) return project
  const next = structuredClone(project)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 standby reference lacks ${name}`)
    if (zone.blocks?.some(block => block.name === 'ventilation_standby_permitted')) continue
    for (const block of template.blocks ?? []) {
      if (added.has(block.name)) (zone.blocks ??= []).push(structuredClone(block))
    }
    for (const [target, oldSource, newSource] of [
      ['ventilation_area_active.u3', 'ventilation_window_eligible.y', 'ventilation_area_window_eligible.y'],
      ['window_primary_minimum.u3', 'window_closed', 'ventilation_area_window_eligible.y'],
      ['occupied_setpoint_requested.u1', 'occupancy_latch.y', 'ventilation_area_presence_or_required.y'],
      ['mode_select_occupied.u2', 'occupancy_latch.y', 'ventilation_area_presence_or_required.y'],
      ['minimum_damper_standby.u3', 'occupancy_latch.y', 'ventilation_area_presence_or_required.y'],
      ['minimum_airflow_standby.u3', 'occupancy_latch.y', 'ventilation_area_presence_or_required.y'],
    ]) {
      const edge = zone.connections?.find(item => edgeEndpoints(item)[1] === target)
      if (edge && edgeEndpoints(edge)[0] === oldSource) {
        edge.from = newSource
        delete edge.from_block
        delete edge.from_output
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      if (!(zone.connections ?? []).some(existing => edgeEndpoints(existing)[1] === to))
        (zone.connections ??= []).push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...project.layout }
  return next
}
