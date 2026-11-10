import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'economizer_use_teaching_demand_gate',
  'economizer_demand_gate_bypass',
  'economizer_demand_gate_select',
])

export function needsG36OptionalDemandGateMigration(saved: Project): boolean {
  const isG36 = saved.graph.name === 'FourPipeG36Controls' ||
    !!saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')
  return isG36 && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_demand_gate_select'))
}

/** Retain the former return-above-supply gate for saved controls. */
export function migrateG36OptionalDemandGate(saved: Project, reference: Block): Project {
  if (!needsG36OptionalDemandGateMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 demand-gate migration requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === 'economizer_demand_gate_select')) continue
    const old = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'free_cooling_useful.u2')
    if (old.length !== 1) throw new Error(`G36 demand-gate migration requires one ${name} source wire`)
    const previousSource = edgeEndpoints(old[0])[0]
    ahu.connections = ahu.connections.filter(edge => edge !== old[0])
    for (const block of template.blocks ?? []) {
      if (added.has(block.name) && !ahu.blocks.some(current => current.name === block.name)) {
        const copy = structuredClone(block)
        if (copy.name === 'economizer_use_teaching_demand_gate') copy.parameters![0].value = true
        ahu.blocks.push(copy)
      }
    }
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'economizer_demand_gate_select.u1') continue
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
    }
    ahu.connections.push({ from: previousSource, to: 'economizer_demand_gate_select.u1' })
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
