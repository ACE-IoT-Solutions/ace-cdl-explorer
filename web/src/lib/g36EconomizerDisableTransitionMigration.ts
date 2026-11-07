import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const removed = new Set([
  'economizer_enabled.y→outdoor_air_select.u3',
  'return_damper_physical_limit.y→return_air_damper_position',
])

export function needsG36EconomizerDisableTransitionMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_return_transition_select'))
}

/** Add the timed final damper commands while retaining tuned PI and limit blocks. */
export function migrateG36EconomizerDisableTransition(saved: Project, reference: Block): Project {
  if (!needsG36EconomizerDisableTransitionMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 economizer disable transition requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === 'economizer_return_transition_select')) continue
    for (const block of template.blocks ?? []) {
      if ((block.name === 'economizer_disable' ||
          block.name.startsWith('economizer_outdoor_') && block.name.includes('transition') ||
          block.name.startsWith('economizer_return_') && block.name.includes('transition') ||
          block.name === 'economizer_outdoor_close_delay' ||
          block.name === 'economizer_return_release_delay') &&
          !ahu.blocks.some(current => current.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    ahu.connections = ahu.connections.filter(edge => !removed.has(edgeEndpoints(edge).join('→')))
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!from.startsWith('economizer_disable.') &&
          !from.startsWith('economizer_outdoor_') &&
          !from.startsWith('economizer_return_') &&
          !to.startsWith('economizer_disable.') &&
          !to.startsWith('economizer_outdoor_') &&
          !to.startsWith('economizer_return_') &&
          to !== 'outdoor_air_select.u3' && to !== 'return_air_damper_position') continue
      const key = `${from}→${to}`
      if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
