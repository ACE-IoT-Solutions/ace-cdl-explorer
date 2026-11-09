import { edgeEndpoints, type Block, type Project } from './types'

const target = 'discharge_valve_pi.trigger'

/** Give each saved G36 reheat loop the pinned OBC enable-triggered PI reset. */
export function needsG36ValvePiResetMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls'
    && !saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return (saved.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController'
    && (zone.blocks?.find(block => block.name === 'discharge_valve_pi')?.block_type
      !== 'Buildings.Controls.OBC.CDL.Reals.PIDWithReset'
      || !zone.connections?.some(edge => {
        const [from, to] = edgeEndpoints(edge)
        return from === 'discharge_valve_enable.y' && to === target
      })))
}

export function migrateG36ValvePiReset(saved: Project, reference: Block): Project {
  if (!needsG36ValvePiResetMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const pi = zone.blocks?.find(block => block.name === 'discharge_valve_pi')
    const template = reference.blocks?.find(block => block.name === zone.name)
      ?.blocks?.find(block => block.name === 'discharge_valve_pi')
    if (!pi || !template) throw new Error(`G36 reheat PI migration requires ${zone.name}.discharge_valve_pi`)
    pi.block_type = template.block_type
    pi.inputs = structuredClone(template.inputs)
    for (const parameter of template.parameters ?? []) {
      if (!pi.parameters?.some(existing => existing.name === parameter.name)) {
        (pi.parameters ??= []).push(structuredClone(parameter))
      }
    }
    if (!zone.connections?.some(edge => edgeEndpoints(edge)[1] === target)) {
      (zone.connections ??= []).push({from: 'discharge_valve_enable.y', to: target})
    }
  }
  return next
}
