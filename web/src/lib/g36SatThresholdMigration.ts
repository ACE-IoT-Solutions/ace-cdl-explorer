import { edgeEndpoints, type Block, type Project } from './types'

const zoneName = (index: number) => `zone_${String(index).padStart(2, '0')}`
const thresholds = ['two', 'three'] as const

export function needsG36SatThresholdMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return Array.from({ length: 10 }, (_, index) => saved.graph.blocks?.find(block =>
    block.name === zoneName(index + 1))).some(zone => thresholds.some(level => {
    const comparator = zone?.blocks?.find(block => block.name === `sat_${level}_error_high`)
    const delay = zone?.blocks?.find(block => block.name === `sat_${level}_error_delay`)
    return comparator && (!comparator.block_type.endsWith('Reals.GreaterThreshold') ||
      delay?.parameters?.some(parameter => parameter.name === 'delayOnInit' && parameter.value === true))
  }))
}

/** Keep tuned thresholds while moving the SAT request comparisons into hysteretic CDL blocks. */
export function migrateG36SatThresholds(saved: Project, reference: Block): Project {
  if (!needsG36SatThresholdMigration(saved)) return saved
  const next = structuredClone(saved)
  for (let index = 1; index <= 10; index++) {
    const zone = next.graph.blocks?.find(block => block.name === zoneName(index))
    const template = reference.blocks?.find(block => block.name === zoneName(index))
    if (!zone || !template) throw new Error(`SAT threshold migration requires ${zoneName(index)}`)
    for (const level of thresholds) {
      const name = `sat_${level}_error_high`
      const currentIndex = zone.blocks?.findIndex(block => block.name === name) ?? -1
      if (currentIndex < 0) continue
      const current = zone.blocks![currentIndex]
      const target = template.blocks?.find(block => block.name === name)
      if (!target) throw new Error(`SAT threshold migration requires ${name}`)
      if (!current.block_type.endsWith('Reals.GreaterThreshold')) {
        const replacement = structuredClone(target)
        const oldConstant = zone.blocks?.find(block => block.name === `sat_${level}_error_threshold`)
        const tuned = oldConstant?.parameters?.find(parameter => parameter.name === 'k')?.value
        if (typeof tuned === 'number') replacement.parameters!.find(parameter => parameter.name === 't')!.value = tuned
        replacement.notes = current.notes ?? replacement.notes
        zone.blocks![currentIndex] = replacement
        zone.blocks = zone.blocks!.filter(block => block.name !== `sat_${level}_error_threshold`)
        const oldSource = zone.connections?.find(edge => edgeEndpoints(edge)[1] === `${name}.u1`)
        const source = oldSource ? edgeEndpoints(oldSource)[0] : 'sat_cooling_error.y'
        zone.connections = (zone.connections ?? []).filter(edge => {
          const to = edgeEndpoints(edge)[1]
          return to !== `${name}.u1` && to !== `${name}.u2` && to !== `${name}.u`
        })
        zone.connections.push({ from: source, to: `${name}.u` })
      }
      const delay = zone.blocks?.find(block => block.name === `sat_${level}_error_delay`)
      const initial = delay?.parameters?.find(parameter => parameter.name === 'delayOnInit')
      if (initial) initial.value = false
    }
  }
  return next
}
