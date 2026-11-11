import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export const climateCutoffBlockNames = [
  'economizer_climate_zone_group', 'economizer_climate_code_real',
  'economizer_climate_low_boundary', 'economizer_climate_mid_boundary',
  'economizer_climate_low_group', 'economizer_climate_mid_group',
  'economizer_climate_cutoff_18', 'economizer_climate_cutoff_21',
  'economizer_climate_cutoff_24', 'economizer_climate_mid_select',
  'economizer_climate_cutoff_select', 'economizer_use_climate_cutoff',
  'economizer_effective_fixed_cutoff',
]
const added = new Set(climateCutoffBlockNames)
const target = 'economizer_fixed_dry_bulb_error.u2'
const stockManual = 'economizer_fixed_dry_bulb_cutoff.y'

export function needsG36ClimateCutoff(saved: Project): boolean {
  if (!saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return [1, 2].some(index => {
    const ahu = saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    return ahu?.blocks?.some(block => block.name === 'economizer_fixed_dry_bulb_error') &&
      !ahu.blocks.some(block => block.name === 'economizer_effective_fixed_cutoff')
  })
}

/** Add climate mapping while preserving a manually tuned cutoff or edited driver. */
export function migrateG36ClimateCutoff(saved: Project, reference: Block): Project {
  if (!needsG36ClimateCutoff(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const ahu = next.graph.blocks?.find(block => block.name === `ahu_${index}`)
    const template = reference.blocks?.find(block => block.name === `ahu_${index}`)
    if (!ahu || !template) throw new Error(`G36 climate cutoff requires AHU ${index}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === 'economizer_effective_fixed_cutoff')) continue
    const prior = ahu.connections.find(edge => edgeEndpoints(edge)[1] === target)
    if (!prior) throw new Error(`G36 climate cutoff requires AHU ${index} fixed-cutoff driver`)
    const previousSource = edgeEndpoints(prior)[0]
    const manualValue = ahu.blocks.find(block => block.name === 'economizer_fixed_dry_bulb_cutoff')
      ?.parameters?.find(parameter => parameter.name === 'k')?.value
    for (const name of climateCutoffBlockNames) {
      if (ahu.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 climate cutoff reference lacks ${name}`)
      ahu.blocks.push(structuredClone(block))
    }
    const selector = ahu.blocks.find(block => block.name === 'economizer_use_climate_cutoff')!
    if (previousSource !== stockManual || manualValue !== 24) {
      selector.parameters!.find(parameter => parameter.name === 'k')!.value = false
    }
    ahu.connections = ahu.connections.filter(edge => edge !== prior)
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      if (to === 'economizer_effective_fixed_cutoff.u2' || to === target) continue
      const key = `${from}→${to}`
      if (!present.has(key) && !ahu.connections.some(current => edgeEndpoints(current)[1] === to)) {
        ahu.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    ahu.connections.push({ from: previousSource, to: 'economizer_effective_fixed_cutoff.u2' })
    ahu.connections.push({ from: 'economizer_effective_fixed_cutoff.y', to: target })
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
