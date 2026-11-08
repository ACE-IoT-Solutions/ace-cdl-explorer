import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = (name: string): boolean =>
  /^zone_status_[1-5]$/.test(name) ||
  /^(warmup|cooldown)_status_(gate|any_[2-5])$/.test(name)
const obsolete = (name: string): boolean =>
  name === 'setback_end_margin' || name === 'setup_end_margin' ||
  /^(cold|hot|warm_end|cool_end|warm_end_target|cool_end_target|cold_window|hot_window|warm_end_window|cool_end_window)_[1-5]$/.test(name)

export function needsG36ZoneStatusMigration(saved: Project): boolean {
  return [1, 2].some(index => {
    const group = saved.graph.blocks?.find(block => block.name === `zone_group_${index}`)
    return !!group && !group.blocks?.some(block => block.name === 'warmup_status_gate')
  })
}

/** Put OBC temperature and window status on the actual group decision paths. */
export function migrateG36ZoneStatus(saved: Project, reference: Block): Project {
  if (!needsG36ZoneStatusMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `zone_group_${index}`
    const group = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!group || !template) throw new Error(`G36 zone-status migration requires ${name}`)
    group.blocks ??= []
    group.connections ??= []
    if (group.blocks.some(block => block.name === 'warmup_status_gate')) continue
    for (const block of template.blocks ?? []) {
      if (added(block.name) && !group.blocks.some(current => current.name === block.name))
        group.blocks.push(structuredClone(block))
    }
    const margin = (name: string): number | undefined => {
      const value = group.blocks?.find(block => block.name === name)
        ?.parameters?.find(parameter => parameter.name === 'k')?.value
      return typeof value === 'number' && Number.isFinite(value) ? value : undefined
    }
    for (const [source, hysteresis] of [
      ['setback_end_margin', 'hys3'], ['setup_end_margin', 'hys4'],
    ]) {
      const value = margin(source)
      if (value === undefined) continue
      for (let zone = 1; zone <= 5; zone++) {
        const threshold = group.blocks.find(block => block.name === `zone_status_${zone}`)
          ?.blocks?.find(block => block.name === hysteresis)
          ?.parameters?.find(parameter => parameter.name === 'uHigh')
        if (threshold) threshold.value = value
      }
    }
    const preservedCandidateSources = new Map<string, string>()
    const rewritten = group.connections.flatMap(edge => {
      const [from, to] = edgeEndpoints(edge)
      for (const mode of ['warmup', 'cooldown']) {
        if (to === `${mode}_candidate.u1`) {
          preservedCandidateSources.set(mode, from)
          return []
        }
      }
      for (let zone = 1; zone <= 5; zone++) {
        for (const [selector, output] of [
          ['cold_window', 'yUnoHeaHig'],
          ['hot_window', 'yHigUnoCoo'],
          ['warm_end_window', 'yEndSetBac'],
          ['cool_end_window', 'yEndSetUp'],
        ]) {
          if (from === `${selector}_${zone}.y`)
            return [{ from: `zone_status_${zone}.${output}`, to }]
        }
      }
      if (obsolete(from.split('.')[0]) || obsolete(to.split('.')[0])) return []
      const match = /^optimal_(warmup|cooldown)_([1-5])_predictor\.seconds$/.exec(from)
      if (match) {
        const output = match[1] === 'warmup' ? 'yWarTim' : 'yCooTim'
        return [{ from: `zone_status_${match[2]}.${output}`, to }]
      }
      return [edge]
    })
    group.connections = rewritten
    group.blocks = group.blocks.filter(block => !obsolete(block.name))
    for (const mode of ['warmup', 'cooldown']) {
      const prior = preservedCandidateSources.get(mode)
      if (!prior) throw new Error(`G36 zone-status migration requires ${name}.${mode}_candidate.u1`)
      group.connections.push({ from: prior, to: `${mode}_status_gate.u1` })
    }
    const occupied = new Map(group.connections.map(edge => {
      const [from, to] = edgeEndpoints(edge)
      return [to, from]
    }))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!added(from.split('.')[0]) && !added(to.split('.')[0])) continue
      if (to.endsWith('_status_gate.u1')) continue
      if (occupied.has(to)) continue
      group.connections.push(structuredClone(edge))
      occupied.set(to, from)
    }
    group.notes = `${group.notes ?? ''} OBC zone-status composites now qualify optimal-start requests.`.trim()
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
