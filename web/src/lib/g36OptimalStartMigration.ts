import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const id = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const isAdded = (name: string) => name.startsWith('optimal_')
const replacedTargets = new Set([
  'warmup_seconds_per_k.u', 'cooldown_seconds_per_k.u',
  'warmup_time_capped.u1', 'cooldown_time_capped.u1',
])
const isLearnerActive = (target: string) => /^optimal_(warmup|cooldown)_[1-5]_learner\.active$/.test(target)

/** Replace the fixed group estimate with per-zone CDL recovery learning. */
export function migrateG36OptimalStart(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls' ||
      !saved.graph.blocks?.some(block => block.name === 'zone_group_1') ||
      saved.graph.blocks.find(block => block.name === 'zone_group_1')?.blocks?.some(block => block.name === 'optimal_warmup_active')) return saved
  const next = structuredClone(saved)
  for (const group of next.graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    const template = reference.blocks?.find(block => block.name === group.name)
    if (!template) throw new Error(`G36 optimal start migration requires ${group.name}`)
    group.blocks ??= []
    group.connections ??= []
    const existing = new Set(group.blocks.map(block => block.name))
    for (const block of template.blocks ?? []) {
      if (isAdded(block.name) && !existing.has(block.name)) group.blocks.push(structuredClone(block))
    }
    group.connections = group.connections.filter(edge => {
      const target = edgeEndpoints(edge)[1]
      return !replacedTargets.has(target) && !isLearnerActive(target)
    })
    const wires = new Set(group.connections.map(id))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!isAdded(from.split('.')[0]) && !isAdded(to.split('.')[0]) && !replacedTargets.has(to)) continue
      if (!wires.has(id(edge))) {
        group.connections.push(structuredClone(edge))
        wires.add(id(edge))
      }
    }
    group.notes = `${group.notes ?? ''} Each closed-window zone learns a recovery slope in editable CDL; the longest sampled prediction sets group optimal start.`.trim()
  }
  next.graph.notes = `${next.graph.notes ?? ''} Per-zone CDL recovery learners now determine optimal start.`.trim()
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
