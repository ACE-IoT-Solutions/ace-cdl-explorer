import { edgeEndpoints, type Block, type Project } from './types'

const find = (block: Block, name: string) => block.blocks?.find(child => child.name === name)
const id = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const has = (block: Block, from: string, to: string) =>
  block.connections?.some(edge => id(edge) === `${from}→${to}`) ?? false
const learnerName = (mode: string, zone: number) => `optimal_${mode}_${zone}_learner`
const addedBlocks = new Set(['latest_completed_rate', 'zero_sampled_need', 'start_required', 'eligible_start', 'daily_sample'])
const retiredBlocks = new Set(['daily_completion', 'daily_completion_fall', 'daily_rate'])

function completionTriggeredLearner(learner: Block): boolean {
  return !learner.inputs?.some(port => port.name === 'sampled_need') &&
    has(learner, 'rate_sample.y', 'rate_mean.u') &&
    has(learner, 'completion_edge.y', 'rate_mean.trigger')
}

function completionQualifiedDailyLearner(learner: Block): boolean {
  if (learner.inputs?.some(port => port.name === 'sampled_need')) return false
  const expected = [
    'completion_edge.y→daily_completion.u', 'midnight→daily_completion.clr',
    'daily_completion.y→daily_completion_fall.u',
    'latest_completed_rate.y→daily_rate.u1', 'fallback_rate→daily_rate.u2',
    'daily_completion_fall.y→daily_rate.u3', 'daily_rate.y→rate_mean.u',
    'daily_completion_fall.y→rate_mean.trigger',
  ]
  const relevant = learner.connections?.filter(edge => edgeEndpoints(edge).some(end =>
    retiredBlocks.has(end.split('.')[0]))).map(id) ?? []
  return relevant.length === expected.length && expected.every(wire => relevant.includes(wire))
}

const legacyLearner = (learner: Block) =>
  completionTriggeredLearner(learner) || completionQualifiedDailyLearner(learner)

export function needsG36DailyRecoveryMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return (saved.graph.blocks ?? []).some(group => /^zone_group_[12]$/.test(group.name) &&
    ['warmup', 'cooldown'].some(mode => Array.from({ length: 5 }, (_, index) =>
      find(group, learnerName(mode, index + 1))).some(learner => learner && legacyLearner(learner))))
}

/** Qualify the daily mean from sampled start need while preserving user-tuned learners. */
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
      for (const port of sample.inputs ?? []) {
        if (['midnight', 'sampled_need'].includes(port.name) && !learner.inputs.some(existing => existing.name === port.name))
          learner.inputs.push(structuredClone(port))
      }
      learner.blocks = learner.blocks.filter(block => !retiredBlocks.has(block.name))
      for (const block of sample.blocks ?? []) {
        if (addedBlocks.has(block.name) && !find(learner, block.name)) learner.blocks.push(structuredClone(block))
      }
      learner.connections = learner.connections.filter(edge => {
        const [from, to] = edgeEndpoints(edge)
        return !retiredBlocks.has(from.split('.')[0]) && !retiredBlocks.has(to.split('.')[0]) &&
          !['rate_sample.y→rate_mean.u', 'completion_edge.y→rate_mean.trigger'].includes(id(edge))
      })
      const wires = new Set(learner.connections.map(id))
      for (const edge of sample.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (['midnight', 'sampled_need'].includes(from) || addedBlocks.has(from.split('.')[0]) || addedBlocks.has(to.split('.')[0])) {
          if (!wires.has(id(edge))) learner.connections.push(structuredClone(edge))
        }
      }
      for (const groupWire of [
        { from: 'optimal_daily_sample.y', to: `${name}.midnight` },
        { from: `optimal_${mode}_${zone}_predictor.sampled_error`, to: `${name}.sampled_need` },
      ]) {
        if (!group.connections.some(edge => id(edge) === id(groupWire))) group.connections.push(groupWire)
      }
      learner.notes = `${learner.notes ?? ''}\nThe daily sample is qualified by positive horizon error and a closed window.`.trim()
      changed = true
    }
    if (changed && !find(group, 'optimal_daily_sample')) {
      group.blocks!.push(structuredClone(find(template, 'optimal_daily_sample')!))
    }
  }
  return next
}
