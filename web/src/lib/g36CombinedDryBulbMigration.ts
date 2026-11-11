import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'economizer_use_combined_dry_bulb',
  'economizer_combined_dry_bulb_trip',
  'economizer_dry_bulb_device_select',
])

export function needsG36CombinedDryBulbMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_dry_bulb_device_select'))
}

/** Insert the combined device after the existing high-limit choice, preserving its input. */
export function migrateG36CombinedDryBulb(saved: Project, reference: Block): Project {
  if (!needsG36CombinedDryBulbMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 combined dry-bulb migration requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === 'economizer_dry_bulb_device_select')) continue
    const old = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'economizer_high_limit_hold.u')
    if (old.length !== 1) throw new Error(`G36 combined dry-bulb migration requires one ${name} high-limit wire`)
    const previousSource = edgeEndpoints(old[0])[0]
    ahu.connections = ahu.connections.filter(edge => edge !== old[0])
    for (const block of template.blocks ?? []) {
      if (added.has(block.name) && !ahu.blocks.some(current => current.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'economizer_dry_bulb_device_select.u3' ||
          to === 'economizer_high_limit_device_select.u3') continue
      if (to.includes('.') && !ahu.blocks.some(block => block.name === to.split('.')[0]) &&
          !added.has(to.split('.')[0])) continue
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
    }
    ahu.connections.push({ from: previousSource, to: 'economizer_dry_bulb_device_select.u3' })
    ahu.connections.push({ from: 'economizer_dry_bulb_device_select.y', to: 'economizer_high_limit_hold.u' })
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
