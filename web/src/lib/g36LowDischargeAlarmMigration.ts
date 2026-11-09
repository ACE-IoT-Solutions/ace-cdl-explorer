import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36LowDischargeAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => {
        const zone = saved.graph.blocks?.find(block => block.name === name)
        return !zone?.blocks?.some(block => block.name === 'low_discharge_priority') ||
          !zone.blocks?.some(block => block.name === 'low_discharge_fan_and_plant') ||
          zone.connections?.some(edge => edgeEndpoints(edge).join('→') ===
            'low_discharge_three_condition.y→low_discharge_three_delay.u')
      })
}

/** Add measured-temperature diagnosis without replacing existing zone tuning. */
export function migrateG36LowDischargeAlarms(saved: Project, reference: Block): Project {
  if (!needsG36LowDischargeAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const status = reference.inputs?.find(port => port.name === 'hot_water_plant_proven')
  if (!status) throw new Error('G36 low discharge alarm migration requires physical hot-water status')
  if (!next.graph.inputs.some(port => port.name === status.name)) next.graph.inputs.push(structuredClone(status))
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 low discharge alarm migration requires ${name}`)
    zone.inputs ??= []
    zone.outputs ??= []
    zone.blocks ??= []
    zone.connections ??= []
    const innerStatus = template.inputs?.find(port => port.name === 'hot_water_plant_proven')
    const output = template.outputs?.find(port => port.name === 'low_discharge_temperature_alarm_level')
    if (!innerStatus || !output) throw new Error(`G36 low discharge alarm migration requires ${name} ports`)
    if (!zone.inputs.some(port => port.name === innerStatus.name)) zone.inputs.push(structuredClone(innerStatus))
    if (!zone.outputs.some(port => port.name === output.name)) zone.outputs.push(structuredClone(output))
    for (const block of template.blocks ?? []) {
      if (block.name.startsWith('low_discharge_') && !zone.blocks.some(current => current.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    const edges = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!to.startsWith('low_discharge_')) continue
      const key = `${from}→${to}`
      if (!edges.has(key)) { zone.connections.push(structuredClone(edge)); edges.add(key) }
    }
    const timedAlarmTargets = new Set([
      'low_discharge_three_delay.u', 'low_discharge_two_delay.u',
      'low_discharge_three_condition.u2', 'low_discharge_two_condition.u2',
      'low_discharge_moderate.u2', 'low_discharge_priority.u2',
      'low_discharge_occupied_and_plant.u2',
    ])
    zone.connections = zone.connections.filter(edge => !timedAlarmTargets.has(edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      if (timedAlarmTargets.has(edgeEndpoints(edge)[1])) zone.connections.push(structuredClone(edge))
    }
    const point = `${name}_low_discharge_temperature_alarm_level`
    const rootOutput = reference.outputs?.find(port => port.name === point)
    if (!rootOutput) throw new Error(`G36 low discharge alarm migration requires ${point}`)
    if (!next.graph.outputs.some(port => port.name === point)) next.graph.outputs.push(structuredClone(rootOutput))
    const rootEdges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to !== point && to !== `${name}.hot_water_plant_proven`) continue
      const key = `${from}→${to}`
      if (!rootEdges.has(key)) { next.graph.connections.push(structuredClone(edge)); rootEdges.add(key) }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
