import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'economizer_use_teaching_low_limit',
  'economizer_low_limit_bypass',
  'economizer_low_limit_select',
])

export function needsG36OptionalLowLimitMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_low_limit_select') ||
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.find(block => block.name === 'economizer_mixed_air_pi')
      ?.inputs?.some(port => port.name === 'trigger'))
}

/** Keep the old 12 °C behavior for saved projects; new samples bypass it by default. */
export function migrateG36OptionalLowLimit(saved: Project, reference: Block): Project {
  if (!needsG36OptionalLowLimitMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 optional low-limit migration requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (!ahu.blocks.some(block => block.name === 'economizer_low_limit_select')) {
      const old = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'economizer_temperature_ok.u1')
      if (old.length !== 1) throw new Error(`G36 optional low-limit migration requires one ${name} temperature wire`)
      const previousSource = edgeEndpoints(old[0])[0]
      ahu.connections = ahu.connections.filter(edge => edge !== old[0])
      for (const block of template.blocks ?? []) {
        if (added.has(block.name) && !ahu.blocks.some(current => current.name === block.name)) {
          const copy = structuredClone(block)
          if (copy.name === 'economizer_use_teaching_low_limit') copy.parameters![0].value = true
          ahu.blocks.push(copy)
        }
      }
      const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
      for (const edge of template.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (to === 'economizer_low_limit_select.u1') continue
        if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
        const key = `${from}→${to}`
        if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
      }
      ahu.connections.push({ from: previousSource, to: 'economizer_low_limit_select.u1' })
    }
    const pi = ahu.blocks.find(block => block.name === 'economizer_mixed_air_pi')
    const referencePi = template.blocks?.find(block => block.name === 'economizer_mixed_air_pi')
    if (!pi || !referencePi) throw new Error(`G36 economizer PI reset migration requires ${name}`)
    if (!pi.inputs?.some(port => port.name === 'trigger')) {
      pi.block_type = referencePi.block_type
      pi.inputs ??= []
      pi.inputs.push(structuredClone(referencePi.inputs!.find(port => port.name === 'trigger')!))
      pi.parameters ??= []
      for (const parameter of referencePi.parameters ?? []) {
        if (!pi.parameters.some(current => current.name === parameter.name)) pi.parameters.push(structuredClone(parameter))
      }
      if (!ahu.connections.some(edge => edgeEndpoints(edge).join('→') === 'economizer_enabled.y→economizer_mixed_air_pi.trigger')) {
        ahu.connections.push({ from: 'economizer_enabled.y', to: 'economizer_mixed_air_pi.trigger' })
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
