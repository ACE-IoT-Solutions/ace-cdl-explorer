import { edgeEndpoints, type Block, type Project } from './types'

/** Upgrade existing G36 VAV feedback loops while keeping their tuned PI values. */
export function migrateG36DamperPiReset(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const zones = saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController') ?? []
  if (!zones.length || zones.every(zone => {
    const pi = zone.blocks?.find(block => block.name === 'damper_flow_pi')
    return pi?.block_type.endsWith('PIDWithReset') && (zone.connections ?? []).some(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === 'supply_fan_proven' && to === 'damper_flow_pi.trigger'
    })
  })) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const pi = zone.blocks?.find(block => block.name === 'damper_flow_pi')
    const template = reference.blocks?.find(block => block.name === zone.name)
      ?.blocks?.find(block => block.name === 'damper_flow_pi')
    if (!pi || !template) throw new Error(`G36 damper PI migration requires ${zone.name}`)
    pi.block_type = template.block_type
    pi.inputs = structuredClone(template.inputs)
    for (const parameter of template.parameters ?? []) {
      if (!pi.parameters?.some(existing => existing.name === parameter.name)) {
        (pi.parameters ??= []).push(structuredClone(parameter))
      }
    }
    pi.notes = pi.notes?.trim() ? pi.notes : template.notes
    if (!(zone.connections ?? []).some(edge => edgeEndpoints(edge)[1] === 'damper_flow_pi.trigger')) {
      (zone.connections ??= []).push({ from: 'supply_fan_proven', to: 'damper_flow_pi.trigger' })
    }
  }
  return next
}
