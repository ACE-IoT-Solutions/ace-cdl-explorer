import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const isGroup = (block: Block) => /^zone_group_[12]$/.test(block.name)
const oldPredictionBlock = (name: string) =>
  (name.startsWith('optimal_') && !name.startsWith('optimal_warmup_longest_') && !name.startsWith('optimal_cooldown_longest_')) ||
  name === 'warmup_seconds_per_k' || name === 'cooldown_seconds_per_k'
const newPredictionBlock = (name: string) => name.startsWith('optimal_start_') ||
  /^optimal_(warmup|cooldown)_[1-5]_obc$/.test(name)

export function needsG36ObcOptimalStartMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return (saved.graph.blocks ?? []).filter(isGroup).some(group =>
    !(group.blocks ?? []).some(block => block.name === 'optimal_warmup_1_obc'))
}

/** Replace the former learner and predictor path while retaining user tuning. */
export function migrateG36ObcOptimalStart(saved: Project, reference: Block): Project {
  if (!needsG36ObcOptimalStartMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const group of (next.graph.blocks ?? []).filter(isGroup)) {
    if (group.blocks?.some(block => block.name === 'optimal_warmup_1_obc')) continue
    const template = reference.blocks?.find(block => block.name === group.name)
    if (!template) throw new Error(`OBC optimal-start migration requires ${group.name}`)
    const oldBlocks = group.blocks ?? []
    const parameter = (blockName: string, key: string): number | undefined => {
      const value = oldBlocks.find(block => block.name === blockName)?.parameters?.find(item => item.name === key)?.value
      return typeof value === 'number' && Number.isFinite(value) ? value : undefined
    }
    const maximum = parameter('maximum_start_seconds', 'k') ?? 10800
    const rateFloor: Record<string, number> = {}
    for (const mode of ['warmup', 'cooldown']) {
      const seconds = parameter(`${mode}_seconds_per_k`, 'k')
      rateFloor[mode] = seconds && seconds > 0 ? 1 / seconds : 1 / 3600
    }
    const days: Record<string, number> = {}
    for (const mode of ['warmup', 'cooldown']) for (let zone = 1; zone <= 5; zone++) {
      const learner = oldBlocks.find(block => block.name === `optimal_${mode}_${zone}_learner`)
      const value = learner?.blocks?.find(block => block.name === 'rate_mean')?.parameters?.find(item => item.name === 'n')?.value
      if (typeof value === 'number' && Number.isInteger(value) && value > 0)
        days[`${mode}_${zone}`] = value
    }
    group.blocks = oldBlocks.filter(block => !oldPredictionBlock(block.name))
    group.connections = (group.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !oldPredictionBlock(from.split('.')[0]) && !oldPredictionBlock(to.split('.')[0])
    })
    const additions = (template.blocks ?? []).filter(block => newPredictionBlock(block.name))
    if (additions.length !== 13) throw new Error(`OBC optimal-start migration requires 13 new blocks in ${group.name}`)
    for (const block of additions) {
      const copy = structuredClone(block)
      if (copy.name === 'optimal_start_horizon_offset')
        copy.parameters!.find(item => item.name === 'p')!.value = -maximum
      const match = copy.name.match(/^optimal_(warmup|cooldown)_([1-5])_obc$/)
      if (match) {
        const [, mode, zone] = match
        const child = (name: string) => copy.blocks!.find(item => item.name === name)!
        child('maxStaTim').parameters!.find(item => item.name === 'k')!.value = maximum
        child('truHol').parameters!.find(item => item.name === 'trueHoldDuration')!.value = maximum + 11 * 3600
        child('defTemSlo').parameters!.find(item => item.name === 'k')!.value = rateFloor[mode]
        const count = days[`${mode}_${zone}`]
        if (count) child('samTemSloAve').parameters!.find(item => item.name === 'n')!.value = count
      }
      group.blocks.push(copy)
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!newPredictionBlock(from.split('.')[0]) && !newPredictionBlock(to.split('.')[0])) continue
      if (!group.connections.some(existing => edgeEndpoints(existing)[1] === to))
        group.connections.push(structuredClone(edge))
    }
    group.notes = `${group.notes ?? ''} Pinned OBC optimal-start CDL now calculates each zone's warm-up and cooldown time.`.trim()
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
