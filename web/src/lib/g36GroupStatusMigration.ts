import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36GroupStatusMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return [1, 2].some(index => saved.graph.blocks?.find(block => block.name === `zone_group_${index}`)
    ?.blocks?.every(block => block.name !== 'obc_group_status'))
}

/** Add the pinned OBC aggregate while keeping zone and equipment tuning. */
export function migrateG36GroupStatus(saved: Project, reference: Block): Project {
  if (!needsG36GroupStatusMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `zone_group_${index}`
    const group = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const status = template?.blocks?.find(block => block.name === 'obc_group_status')
    if (!group || !template || !status) throw new Error(`G36 GroupStatus migration requires ${name}`)
    group.blocks ??= []
    group.connections ??= []
    if (group.blocks.some(block => block.name === 'obc_group_status')) continue
    group.blocks.push(structuredClone(status))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to.startsWith('obc_group_status.')) {
        if (!group.connections.some(existing => edgeEndpoints(existing)[1] === to))
          group.connections.push(structuredClone(edge))
        continue
      }
      if (!from.startsWith('obc_group_status.')) continue
      const existing = group.connections.find(connection => edgeEndpoints(connection)[1] === to)
      if (!existing) {
        group.connections.push(structuredClone(edge))
      } else {
        const [old] = edgeEndpoints(existing)
        if (old === 'group_occupied.y' || to.startsWith('obc_operation_mode.')) {
          existing.from = from
          existing.to = to
        }
      }
    }
    group.notes = `${group.notes ?? ''} Pinned OBC GroupStatus supplies occupied status, recovery extrema, zone counts and setback/setup triggers.`.trim()
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
