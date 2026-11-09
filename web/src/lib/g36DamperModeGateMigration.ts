import { edgeEndpoints, type Block, type Project } from './types'

const oldSource = 'damper_flow_requested.y'
const target = 'damper_feedback_output.u3'
const newBlocks = new Set(['damper_is_unoccupied', 'damper_operation_enabled'])

/** Keep a zero-flow override under PI control while closing the damper in unoccupied mode. */
export function needsG36DamperModeGateMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController' &&
      (zone.connections ?? []).some(edge => {
        const [from, to] = edgeEndpoints(edge)
        return from === oldSource && to === target
      }))
}

export function migrateG36DamperModeGate(saved: Project, reference: Block): Project {
  if (!needsG36DamperModeGateMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const edge = (zone.connections ?? []).find(item => {
      const [from, to] = edgeEndpoints(item)
      return from === oldSource && to === target
    })
    if (!edge) continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`G36 damper mode migration requires ${zone.name}`)
    for (const block of template.blocks ?? []) {
      if (newBlocks.has(block.name) && !zone.blocks?.some(existing => existing.name === block.name)) {
        (zone.blocks ??= []).push(structuredClone(block))
      }
    }
    edge.from = 'damper_operation_enabled.y'
    delete edge.from_block
    delete edge.from_output
    for (const connection of template.connections ?? []) {
      const [from, to] = edgeEndpoints(connection)
      if (!newBlocks.has(from.split('.')[0]) && !newBlocks.has(to.split('.')[0])) continue
      if (to === target) continue
      if (!(zone.connections ?? []).some(existing => edgeEndpoints(existing)[1] === to)) {
        (zone.connections ??= []).push(structuredClone(connection))
      }
    }
  }
  return next
}
