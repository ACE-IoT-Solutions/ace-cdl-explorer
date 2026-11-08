import { edgeEndpoints, type Block, type Project } from './types'

const formerBlocks = new Set([
  'pressure_request_damper_threshold', 'damper_nearly_open',
  'pressure_request_flow_fraction', 'pressure_request_low_flow',
  'pressure_request_condition', 'pressure_request_delay', 'pressure_request_integer',
])

const isZone = (block: Block) => block.block_type === 'VAVBoxController'
const root = (endpoint: string) => endpoint.split('.')[0]

export function needsG36PressureRequestLevelsMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(isZone).some(zone =>
      !zone.blocks?.some(block => block.name === 'pressure_importance_select') ||
      !zone.blocks?.some(block => block.name === 'low_flow_importance_gate'))
}

/** Replace the teaching binary request path, retaining unrelated zone tuning and layout. */
export function migrateG36PressureRequestLevels(saved: Project, reference: Block): Project {
  if (!needsG36PressureRequestLevelsMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(isZone)) {
    if (zone.blocks?.some(block => block.name === 'pressure_importance_select') &&
        zone.blocks?.some(block => block.name === 'low_flow_importance_gate')) continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`Pressure request migration requires ${zone.name}`)
    const additions = (template.blocks ?? []).filter(block =>
      (block.name.startsWith('pressure_') && block.name !== 'pressure_effect') ||
      block.name === 'low_flow_importance_gate')
    if (!additions.some(block => block.name === 'pressure_importance_select') ||
        !additions.some(block => block.name === 'low_flow_importance_gate'))
      throw new Error(`Pressure request migration requires the new CDL branch in ${zone.name}`)
    const addedNames = new Set(additions.map(block => block.name))
    zone.blocks = (zone.blocks ?? []).filter(block => !formerBlocks.has(block.name))
    for (const block of additions) {
      if (!zone.blocks.some(existing => existing.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !formerBlocks.has(root(from)) && !formerBlocks.has(root(to)) &&
        to !== 'pressure_reset_request' &&
        to !== 'low_flow_eligible.u1' && to !== 'low_flow_occupied_gate.u1'
    })
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!addedNames.has(root(to)) && to !== 'pressure_reset_request' &&
          !(from === 'low_flow_importance_gate.y' &&
            (to === 'low_flow_eligible.u1' || to === 'low_flow_occupied_gate.u1'))) continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    zone.notes = `${zone.notes ?? ''} Editable 0–3 duct-pressure requests and zero-importance low-flow alarm suppression.`.trim()
  }
  return next
}
