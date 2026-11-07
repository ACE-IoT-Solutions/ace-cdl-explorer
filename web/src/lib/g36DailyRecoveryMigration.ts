import { edgeEndpoints, type Block, type Project } from './types'

const find = (block: Block, name: string) => block.blocks?.find(child => child.name === name)
const id = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const has = (block: Block, from: string, to: string) =>
  block.connections?.some(edge => id(edge) === `${from}→${to}`) ?? false
const learnerName = (mode: string, zone: number) => `optimal_${mode}_${zone}_learner`
const newBlocks = new Set(['latest_completed_rate', 'daily_completion', 'daily_completion_fall', 'daily_rate'])

function legacyLearner(learner: Block): boolean {
  return !learner.inputs?.some(port => port.name === 'midnight') &&
    has(learner, 'rate_sample.y', 'rate_mean.u') &&
    has(learner, 'completion_edge.y', 'rate_mean.trigger')
}

export function needsG36DailyRecoveryMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return (saved.graph.blocks ?? []).some(group => /^zone_group_[12]$/.test(group.name) &&
    ['warmup', 'cooldown'].some(mode => Array.from({ length: 5 }, (_, index) =>
      find(group, learnerName(mode, index + 1))).some(learner => learner && legacyLearner(learner))))
}

/** Move only the shipped completion-triggered mean path to editable daily CDL blocks. */
export function migrateG36DailyRecovery(saved: Project, reference: Block): Project {
  if (!needsG36DailyRecoveryMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const group of next.graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name)) continue
    const template = find(reference, group.name)
    if (!template) throw new Error(`Missing ${group.name} daily recovery reference`)
    group.connections ??= []
    let changed = false
    for (const mode of ['warmup', 'cooldown']) for (let zone = 1; zone <= 5; zone++) {
      const name = learnerName(mode, zone)
      const learner = find(group, name)
      if (!learner || !legacyLearner(learner)) continue
      const sample = find(template, name)
      if (!sample) throw new Error(`Missing ${name} daily recovery reference`)
      learner.inputs ??= []
      learner.blocks ??= []
      learner.connections ??= []
      learner.inputs.push(structuredClone(sample.inputs!.find(port => port.name === 'midnight')!))
      for (const block of sample.blocks ?? []) {
        if (newBlocks.has(block.name) && !find(learner, block.name)) learner.blocks.push(structuredClone(block))
      }
      learner.connections = learner.connections.filter(edge =>
        !['rate_sample.y→rate_mean.u', 'completion_edge.y→rate_mean.trigger'].includes(id(edge)))
      const wires = new Set(learner.connections.map(id))
      for (const edge of sample.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (from === 'midnight' || newBlocks.has(from.split('.')[0]) || newBlocks.has(to.split('.')[0])) {
          if (!wires.has(id(edge))) learner.connections.push(structuredClone(edge))
        }
      }
      const groupWire = { from: 'optimal_daily_sample.y', to: `${name}.midnight` }
      if (!group.connections.some(edge => id(edge) === id(groupWire))) group.connections.push(groupWire)
      learner.notes = `${learner.notes ?? ''}\nCompleted recovery is sampled at the next daily trigger.`.trim()
      changed = true
    }
    if (changed && !find(group, 'optimal_daily_sample')) {
      group.blocks!.push(structuredClone(find(template, 'optimal_daily_sample')!))
    }
  }
  return next
}
