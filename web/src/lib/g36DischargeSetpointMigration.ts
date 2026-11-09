import { edgeEndpoints, type Block, type Project } from './types'

const zoneBlocks = ['discharge_baseline', 'discharge_heat_detected', 'discharge_heat_hold']
const ahuDelay = (ahu: number) => `ahu_${ahu}_supply_target_previous`

/** Connect the pinned discharge target to the AHU setpoint through sampled CDL state. */
export function needsG36DischargeSetpointMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls'
    && !saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  if ([1, 2].some(ahu => !saved.graph.blocks?.some(block => block.name === ahuDelay(ahu)))) return true
  return (saved.graph.blocks ?? []).some(zone => zone.block_type === 'VAVBoxController'
    && (!zone.inputs?.some(port => port.name === 'supply_air_setpoint')
      || zoneBlocks.some(name => !zone.blocks?.some(block => block.name === name))
      || !zone.connections?.some(edge => {
        const [from, to] = edgeEndpoints(edge)
        return from === 'discharge_baseline.y' && to === 'discharge_target.u2'
      })))
}

export function migrateG36DischargeSetpoint(saved: Project, reference: Block): Project {
  if (!needsG36DischargeSetpointMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.blocks ??= []
  next.graph.connections ??= []
  for (const ahu of [1, 2]) {
    const name = ahuDelay(ahu)
    if (!next.graph.blocks.some(block => block.name === name)) {
      const template = reference.blocks?.find(block => block.name === name)
      if (!template) throw new Error(`G36 reference lacks ${name}`)
      next.graph.blocks.push(structuredClone(template))
    }
    const target = `${name}.u`
    if (!next.graph.connections.some(edge => edgeEndpoints(edge)[1] === target)) {
      next.graph.connections.push({from: `ahu_${ahu}.supply_air_temp`, to: target})
    }
  }
  for (const zone of next.graph.blocks.filter(block => block.block_type === 'VAVBoxController')) {
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`G36 reference lacks ${zone.name}`)
    zone.inputs ??= []
    zone.blocks ??= []
    zone.connections ??= []
    if (!zone.inputs.some(port => port.name === 'supply_air_setpoint')) {
      const port = template.inputs?.find(port => port.name === 'supply_air_setpoint')
      if (!port) throw new Error(`G36 reference lacks ${zone.name}.supply_air_setpoint`)
      zone.inputs.push(structuredClone(port))
    }
    for (const name of zoneBlocks) {
      if (zone.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 reference lacks ${zone.name}.${name}`)
      zone.blocks.push(structuredClone(block))
    }
    const replacements: Record<string, string> = {
      'discharge_span.u2': 'supply_air_setpoint',
      'discharge_reset_target.u1': 'supply_air_setpoint',
      'discharge_target.u2': 'discharge_baseline.y',
      'discharge_target.u3': 'discharge_heat_hold.y',
      'discharge_valve_enable.u1': 'discharge_heat_hold.y',
    }
    for (const [target, source] of Object.entries(replacements)) {
      const edge = zone.connections.find(edge => edgeEndpoints(edge)[1] === target)
      if (!edge) throw new Error(`${zone.name} lacks ${target}`)
      edge.from = source
    }
    const driven = new Set(zone.connections.map(edge => edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!zoneBlocks.includes(to.split('.')[0]) || driven.has(to)) continue
      zone.connections.push({from, to})
      driven.add(to)
    }
    const number = Number(zone.name.slice(-2))
    const ahu = number <= 5 ? 1 : 2
    const target = `${zone.name}.supply_air_setpoint`
    const source = `${ahuDelay(ahu)}.y`
    const boundary = next.graph.connections.find(edge => edgeEndpoints(edge)[1] === target)
    if (boundary) boundary.from = source
    else next.graph.connections.push({from: source, to: target})
  }
  return next
}
