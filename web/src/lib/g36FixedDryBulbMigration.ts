import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'economizer_fixed_dry_bulb_cutoff',
  'economizer_fixed_dry_bulb_error',
  'economizer_fixed_dry_bulb_hysteresis',
  'economizer_use_fixed_dry_bulb',
  'economizer_dry_bulb_high_limit_select',
])

export function needsG36FixedDryBulbMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_dry_bulb_high_limit_select'))
}

/** Add the selectable high-limit device without replacing edited differential tuning. */
export function migrateG36FixedDryBulb(saved: Project, reference: Block): Project {
  if (!needsG36FixedDryBulbMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 fixed dry-bulb migration requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === 'economizer_dry_bulb_high_limit_select')) continue
    for (const block of template.blocks ?? []) {
      if (added.has(block.name) && !ahu.blocks.some(current => current.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    const old = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u')
    if (old.length !== 1) throw new Error(`G36 fixed dry-bulb migration requires one ${name} high-limit wire`)
    const previousSource = edgeEndpoints(old[0])[0]
    ahu.connections = ahu.connections.filter(edge => edge !== old[0])
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'economizer_dry_bulb_high_limit_select.u3') continue
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
    }
    ahu.connections.push({ from: previousSource, to: 'economizer_dry_bulb_high_limit_select.u3' })
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
