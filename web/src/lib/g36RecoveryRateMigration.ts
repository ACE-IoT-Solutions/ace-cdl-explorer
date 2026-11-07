import { edgeEndpoints, type Block, type Project } from './types'

const endpoint = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const has = (block: Block, from: string, to: string) =>
  block.connections?.some(edge => endpoint(edge) === `${from}→${to}`) ?? false
const child = (block: Block, name: string) => block.blocks?.find(item => item.name === name)

function legacyLearner(learner: Block): boolean {
  const floor = child(learner, 'rate_floor')
  return floor?.block_type.split('.').pop() === 'Max' &&
    has(learner, 'rate_mean.y', 'rate_floor.u1') &&
    has(learner, 'fallback_rate', 'rate_floor.u2') &&
    has(learner, 'rate_floor.y', 'rate') &&
    learner.connections?.filter(edge => edgeEndpoints(edge).some(end => end.startsWith('rate_floor.'))).length === 3
}

function legacySelector(group: Block, prefix: string, mode: string): boolean {
  const selected = child(group, `${prefix}_rate_safe`)
  return selected?.block_type.split('.').pop() === 'Max' &&
    has(group, `${prefix}_rate_previous.y`, `${prefix}_rate_safe.u1`) &&
    has(group, `optimal_${mode}_fallback_rate.y`, `${prefix}_rate_safe.u2`)
}

export function needsG36RecoveryRateMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return (saved.graph.blocks ?? []).some(group => /^zone_group_[12]$/.test(group.name) &&
    ['warmup', 'cooldown'].some(mode => Array.from({ length: 5 }, (_, index) => {
      const prefix = `optimal_${mode}_${index + 1}`
      return legacyLearner(child(group, `${prefix}_learner`) ?? { name: '', block_type: '' }) ||
        legacySelector(group, prefix, mode)
    }).some(Boolean)))
}

/** Let completed episodes teach a slower rate while retaining user-edited graph branches. */
export function migrateG36RecoveryRate(saved: Project, reference: Block): Project {
  if (!needsG36RecoveryRateMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const group of next.graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    const template = child(reference, group.name)
    if (!template) throw new Error(`Missing ${group.name} recovery reference`)
    let changedSelector = false
    for (const mode of ['warmup', 'cooldown']) for (let zone = 1; zone <= 5; zone++) {
      const prefix = `optimal_${mode}_${zone}`
      const learner = child(group, `${prefix}_learner`)
      if (learner && legacyLearner(learner)) {
        learner.blocks = learner.blocks!.filter(block => block.name !== 'rate_floor')
        learner.connections = learner.connections!.filter(edge =>
          !edgeEndpoints(edge).some(end => end.startsWith('rate_floor.')))
        learner.connections.push({ from: 'rate_mean.y', to: 'rate' })
        learner.notes = `${learner.notes ?? ''} Learned recovery may be faster or slower than the initial fallback.`.trim()
      }
      if (!legacySelector(group, prefix, mode)) continue
      const selected = child(group, `${prefix}_rate_safe`)! // exact legacy signature checked above
      const replacement = child(template, selected.name)
      const available = child(template, `${prefix}_rate_available`)
      if (!replacement || !available) throw new Error(`Missing ${prefix} recovery selector reference`)
      const oldNotes = selected.notes
      Object.assign(selected, structuredClone(replacement))
      if (oldNotes) selected.notes = `${oldNotes}\n${selected.notes ?? ''}`.trim()
      if (!child(group, available.name)) group.blocks!.push(structuredClone(available))
      group.connections = group.connections!.filter(edge =>
        !edgeEndpoints(edge)[1].startsWith(`${prefix}_rate_safe.`) &&
        !edgeEndpoints(edge)[1].startsWith(`${prefix}_rate_available.`))
      for (const edge of template.connections ?? []) {
        const [, to] = edgeEndpoints(edge)
        if (to.startsWith(`${prefix}_rate_available.`) || to.startsWith(`${prefix}_rate_safe.`)) {
          group.connections.push(structuredClone(edge))
        }
      }
      changedSelector = true
    }
    if (changedSelector && !child(group, 'optimal_zero_rate')) {
      const zero = child(template, 'optimal_zero_rate')
      if (!zero) throw new Error('Missing optimal_zero_rate recovery reference')
      group.blocks!.push(structuredClone(zero))
    }
  }
  return next
}
