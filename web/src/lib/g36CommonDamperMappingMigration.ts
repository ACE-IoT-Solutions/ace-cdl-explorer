import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Map the lower half of the minimum-air PI range over full outdoor-damper travel. */
export function migrateG36CommonDamperMapping(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ([1, 2].every(index => saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    ?.blocks?.some(block => block.name === 'minimum_oa_outdoor_stroke'))) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 common-damper migration requires ${name}`)
    for (const blockName of ['minimum_oa_lower_half', 'minimum_oa_outdoor_stroke']) {
      const child = template.blocks?.find(block => block.name === blockName)
      if (!child) throw new Error(`G36 common-damper reference lacks ${blockName}`)
      if (!ahu.blocks?.some(block => block.name === blockName)) (ahu.blocks ??= []).push(structuredClone(child))
    }
    ahu.connections = (ahu.connections ?? []).filter(edge => !new Set([
      'minimum_oa_pi.y→minimum_oa_active.u1',
      'outdoor_damper_physical_limit.y→return_damper_complement.u2',
    ]).has(edgeEndpoints(edge).join('→')))
    for (const [from, to] of [
      ['minimum_oa_pi.y', 'minimum_oa_lower_half.u1'],
      ['return_damper_pi_midpoint.y', 'minimum_oa_lower_half.u2'],
      ['minimum_oa_lower_half.y', 'minimum_oa_outdoor_stroke.u'],
      ['minimum_oa_outdoor_stroke.y', 'minimum_oa_active.u1'],
      ['outdoor_air_select.y', 'return_damper_complement.u2'],
    ]) {
      if (!ahu.connections.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
        ahu.connections.push({ from, to })
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
