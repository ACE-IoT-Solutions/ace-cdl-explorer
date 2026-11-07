import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const id = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const isAdded = (name: string) => name.startsWith('optimal_')
const replacedTargets = new Set([
  'warmup_seconds_per_k.u', 'cooldown_seconds_per_k.u',
  'warmup_time_capped.u1', 'cooldown_time_capped.u1',
])
const isLearnerActive = (target: string) => /^optimal_(warmup|cooldown)_[1-5]_learner\.active$/.test(target)
const learnerNames = (mode: string, zone: number) => `optimal_${mode}_${zone}_learner`

export function needsG36OptimalStartMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  const groups = saved.graph.blocks?.filter(block => /^zone_group_[12]$/.test(block.name)) ?? []
  if (groups.length !== 2) return false
  return groups.some(group => {
    if (['warmup', 'cooldown'].some(mode =>
      !group.blocks?.some(block => block.name === `optimal_${mode}_active` && block.block_type.endsWith('Latch')))) return true
    return ['warmup', 'cooldown'].some(mode =>
      Array.from({ length: 5 }, (_, index) => group.blocks?.find(block => block.name === learnerNames(mode, index + 1)))
        .some(learner => !learner?.blocks?.some(block => block.name === 'reached_before_timeout')))
  })
}

/** Replace the fixed group estimate with per-zone CDL recovery learning. */
export function migrateG36OptimalStart(saved: Project, reference: Block): Project {
  if (!needsG36OptimalStartMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const group of next.graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    const template = reference.blocks?.find(block => block.name === group.name)
    if (!template) throw new Error(`G36 optimal start migration requires ${group.name}`)
    group.blocks ??= []
    group.connections ??= []
    for (const mode of ['warmup', 'cooldown']) {
      const activeName = `optimal_${mode}_active`
      const oldActive = group.blocks.findIndex(block => block.name === activeName)
      const newActive = template.blocks?.find(block => block.name === activeName)
      if (oldActive >= 0 && newActive) {
        const replacement = structuredClone(newActive)
        const oldNotes = group.blocks[oldActive].notes
        if (oldNotes && oldNotes !== replacement.notes) replacement.notes = `${oldNotes}\n${replacement.notes ?? ''}`.trim()
        group.blocks[oldActive] = replacement
      }
    }
    const existing = new Set(group.blocks.map(block => block.name))
    for (const block of template.blocks ?? []) {
      if (isAdded(block.name) && !existing.has(block.name)) group.blocks.push(structuredClone(block))
    }
    for (const mode of ['warmup', 'cooldown']) for (let zone = 1; zone <= 5; zone++) {
      const name = learnerNames(mode, zone)
      const learner = group.blocks.find(block => block.name === name)
      const referenceLearner = template.blocks?.find(block => block.name === name)
      if (!learner || !referenceLearner) continue
      learner.blocks ??= []
      learner.connections ??= []
      const existingLearnerBlocks = new Set(learner.blocks.map(block => block.name))
      for (const block of referenceLearner.blocks ?? []) {
        if (['not_timed_out', 'reached_before_timeout'].includes(block.name) && !existingLearnerBlocks.has(block.name)) {
          learner.blocks.push(structuredClone(block))
        }
      }
      learner.blocks = learner.blocks.filter(block => block.name !== 'done')
      learner.connections = learner.connections.filter(edge => edgeEndpoints(edge)[1] !== 'active_done.u2' &&
        !edgeEndpoints(edge)[0].startsWith('done.') && !edgeEndpoints(edge)[1].startsWith('done.'))
      const learnerWires = new Set(learner.connections.map(id))
      for (const edge of referenceLearner.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (!['not_timed_out', 'reached_before_timeout'].includes(from.split('.')[0]) &&
            !['not_timed_out', 'reached_before_timeout'].includes(to.split('.')[0]) && to !== 'active_done.u2') continue
        if (!learnerWires.has(id(edge))) {
          learner.connections.push(structuredClone(edge))
          learnerWires.add(id(edge))
        }
      }
      const timer = learner.blocks.find(block => block.name === 'recovery_timer')
      const timerReference = referenceLearner.blocks?.find(block => block.name === 'recovery_timer')
      const timeout = timer?.parameters?.find(parameter => parameter.name === 't')
      if (timeout?.value === 10800 && timerReference?.parameters) timeout.value = 50400
      if (learner.notes !== referenceLearner.notes)
        learner.notes = `${learner.notes ?? ''}\n${referenceLearner.notes ?? ''}`.trim()
    }
    group.connections = group.connections.filter(edge => {
      const target = edgeEndpoints(edge)[1]
      return !replacedTargets.has(target) && !isLearnerActive(target) &&
        !/^optimal_(warmup|cooldown)_active\.(u1|u2)$/.test(target)
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
