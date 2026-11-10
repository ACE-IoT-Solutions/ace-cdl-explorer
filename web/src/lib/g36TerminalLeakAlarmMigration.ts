import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const closedPairs = [
  ['leak_damper_closed_limit', 'leak_damper_command_closed'],
  ['leak_valve_closed_limit', 'leak_valve_position_closed'],
] as const

function needsClosedThresholdUpdate(zone: Block): boolean {
  return closedPairs.some(([constant, comparator]) => {
    const limit = zone.blocks?.find(block => block.name === constant)?.parameters?.find(parameter => parameter.name === 'k')
    const compare = zone.blocks?.find(block => block.name === comparator)
    return Boolean(limit && compare?.block_type.endsWith('Less') &&
      !compare.parameters?.some(parameter => parameter.name === 'h'))
  })
}

export function needsG36TerminalLeakAlarmMigration(saved: Project): boolean {
  const isG36 = saved.graph.name === 'FourPipeG36Controls' ||
    saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')
  return Boolean(isG36) &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => {
        const zone = saved.graph.blocks?.find(block => block.name === name)
        return !zone?.blocks?.some(block => block.name === 'leak_valve_select') || needsClosedThresholdUpdate(zone)
      })
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
    for (const [constant, comparator] of closedPairs) {
      const limit = zone.blocks.find(block => block.name === constant)?.parameters?.find(parameter => parameter.name === 'k')
      const compare = zone.blocks.find(block => block.name === comparator)
      if (compare?.block_type.endsWith('Less') && !compare.parameters?.some(parameter => parameter.name === 'h')) {
        if (limit?.value === 0.005) limit.value = 0.05
        const value = typeof limit?.value === 'number' ? limit.value : 0.05
        compare.parameters ??= []
        compare.parameters.push({ name: 'h', type: 'Real', value: value / 2, unit: 'fraction (0–1)' })
      }
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

/** Time the complete valve-leak condition, as in OBC Alarms.truDel6. */
export function needsG36TerminalLeakTimingMigration(saved: Project): boolean {
  const isG36 = saved.graph.name === 'FourPipeG36Controls' ||
    saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')
  return Boolean(isG36) && (saved.graph.blocks ?? [])
    .filter(block => block.block_type === 'VAVBoxController')
    .some(zone => zone.connections?.some(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === 'leak_valve_position_closed.y' && to === 'leak_valve_closed_delay.u'
    }))
}

export function migrateG36TerminalLeakTiming(saved: Project): Project {
  if (!needsG36TerminalLeakTimingMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(block => block.block_type === 'VAVBoxController')) {
    if (!zone.connections?.some(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === 'leak_valve_position_closed.y' && to === 'leak_valve_closed_delay.u'
    })) continue
    const delay = zone.blocks?.find(block => block.name === 'leak_valve_closed_delay')
    const comparator = zone.blocks?.find(block => block.name === 'leak_valve_temperature_high')
    if (!delay || !comparator) throw new Error(`Valve leak timing migration requires ${zone.name} CDL blocks`)
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [, to] = edgeEndpoints(edge)
      return !['leak_valve_closed_delay.u', 'leak_valve_fan_and_closed.u1',
        'leak_valve_select.u2'].includes(to)
    })
    zone.connections.push(
      {from:'leak_valve_position_closed.y',to:'leak_valve_fan_and_closed.u1'},
      {from:'leak_valve_condition.y',to:'leak_valve_closed_delay.u'},
      {from:'leak_valve_closed_delay.y',to:'leak_valve_select.u2'},
    )
    if (!comparator.parameters?.some(parameter => parameter.name === 'h')) {
      comparator.parameters = [...(comparator.parameters ?? []),
        {name:'h',type:'Real',value:0.25,unit:'K'}]
    }
    delay.notes = `${delay.notes ?? ''} The closed valve, fan proof, and warm discharge must persist together.`.trim()
  }
  return next
}
