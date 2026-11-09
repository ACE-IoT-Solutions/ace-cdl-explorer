import { edgeEndpoints, type Block, type Project } from './types'

const isZone = (block: Block) => block.block_type === 'VAVBoxController'
const root = (endpoint: string) => endpoint.split('.')[0]
const requestBlock = (name: string) =>
  (name.startsWith('pressure_') && name !== 'pressure_effect') ||
  name === 'damper_nearly_open' || name === 'low_flow_importance_gate'

export function needsG36PressureRequestLevelsMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(isZone).some(zone =>
      !zone.blocks?.some(block => block.name === 'pressure_damper_sample'))
}

/** Rebuild only the request branch; retain existing threshold and importance tuning. */
export function migrateG36PressureRequestLevels(saved: Project, reference: Block): Project {
  if (!needsG36PressureRequestLevelsMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(isZone)) {
    if (zone.blocks?.some(block => block.name === 'pressure_damper_sample')) continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`Pressure request migration requires ${zone.name}`)
    const oldBlocks = new Map((zone.blocks ?? []).map(block => [block.name, block]))
    zone.blocks = (zone.blocks ?? []).filter(block => !requestBlock(block.name))
    for (const source of template.blocks ?? []) {
      if (!requestBlock(source.name)) continue
      const block = structuredClone(source)
      const old = oldBlocks.get(block.name)
      if (old) {
        for (const parameter of block.parameters ?? []) {
          const previous = old.parameters?.find(item => item.name === parameter.name)
          if (previous) parameter.value = previous.value
        }
        if (old.notes) block.notes = old.notes
      }
      zone.blocks.push(block)
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !requestBlock(root(from)) && !requestBlock(root(to)) &&
        to !== 'pressure_reset_request' &&
        to !== 'low_flow_eligible.u1' && to !== 'low_flow_occupied_gate.u1'
    })
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!requestBlock(root(to)) && to !== 'pressure_reset_request' &&
          to !== 'low_flow_eligible.u1' && to !== 'low_flow_occupied_gate.u1') continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    zone.notes = `${zone.notes ?? ''} Duct-pressure requests sample damper, setpoint, and measured flow together before the one-minute damper qualification.`.trim()
  }
  return next
}
