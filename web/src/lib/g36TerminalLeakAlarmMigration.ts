import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36TerminalLeakAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => !saved.graph.blocks?.find(block => block.name === name)
        ?.blocks?.some(block => block.name === 'leak_valve_select'))
}

/** Add raw actuator feedback and independent CDL alarms, retaining edited controls. */
export function migrateG36TerminalLeakAlarms(saved: Project, reference: Block): Project {
  if (!needsG36TerminalLeakAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const rootEdges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 terminal leak alarm migration requires ${name}`)
    zone.inputs ??= []
    zone.outputs ??= []
    zone.blocks ??= []
    zone.connections ??= []
    for (const portName of [`${name}_actual_damper_position`, `${name}_actual_reheat_valve_position`]) {
      const point = reference.inputs?.find(port => port.name === portName)
      if (!point) throw new Error(`G36 terminal leak alarm migration requires ${portName}`)
      if (!next.graph.inputs.some(port => port.name === portName)) next.graph.inputs.push(structuredClone(point))
    }
    const valvePort = template.inputs?.find(port => port.name === 'actual_reheat_valve_position')
    if (!valvePort) throw new Error(`G36 terminal leak alarm migration requires ${name} valve position`)
    if (!zone.inputs.some(port => port.name === valvePort.name)) zone.inputs.push(structuredClone(valvePort))
    for (const block of template.blocks ?? []) {
      if (block.name.startsWith('leak_') && !zone.blocks.some(current => current.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    for (const suffix of ['leaking_damper_alarm_level', 'leaking_reheat_valve_alarm_level']) {
      const output = template.outputs?.find(port => port.name === suffix)
      const point = reference.outputs?.find(port => port.name === `${name}_${suffix}`)
      if (!output || !point) throw new Error(`G36 terminal leak alarm migration requires ${name} ${suffix}`)
      if (!zone.outputs.some(port => port.name === suffix)) zone.outputs.push(structuredClone(output))
      if (!next.graph.outputs.some(port => port.name === point.name)) next.graph.outputs.push(structuredClone(point))
    }
    const innerEdges = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!to.startsWith('leak_') && !to.startsWith('leaking_')) continue
      const key = `${from}→${to}`
      if (!innerEdges.has(key)) { zone.connections.push(structuredClone(edge)); innerEdges.add(key) }
    }
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to !== `${name}.actual_reheat_valve_position` &&
          to !== `${name}_leaking_damper_alarm_level` &&
          to !== `${name}_leaking_reheat_valve_alarm_level`) continue
      const key = `${from}→${to}`
      if (!rootEdges.has(key)) { next.graph.connections.push(structuredClone(edge)); rootEdges.add(key) }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
