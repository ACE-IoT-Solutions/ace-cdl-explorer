import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const alarmBlocks = new Set([
  'low_flow_seventy_setpoint', 'low_flow_below_seventy', 'low_flow_eligible',
  'low_flow_occupied_gate', 'low_flow_seventy_condition', 'low_flow_seventy_delay',
  'low_flow_alarm_zero', 'low_flow_alarm_three', 'low_flow_alarm_two',
  'low_flow_alarm_moderate', 'low_flow_alarm_priority',
])

export function needsG36TerminalAirflowAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => {
        const zone = saved.graph.blocks?.find(block => block.name === name)
        return zone && !zone.blocks?.some(block => block.name === 'low_flow_alarm_priority')
      })
}

/** Add the new diagnosis while preserving all existing zone control tuning. */
export function migrateG36TerminalAirflowAlarms(saved: Project, reference: Block): Project {
  if (!needsG36TerminalAirflowAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.outputs ??= []
  next.graph.connections ??= []
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 airflow alarm migration requires ${name}`)
    zone.blocks ??= []
    zone.outputs ??= []
    zone.connections ??= []
    for (const block of template.blocks ?? []) {
      if (alarmBlocks.has(block.name) && !zone.blocks.some(existing => existing.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    const port = template.outputs?.find(port => port.name === 'low_airflow_alarm_level')
    if (!port) throw new Error(`G36 airflow alarm migration requires ${name} alarm output`)
    if (!zone.outputs.some(existing => existing.name === port.name)) zone.outputs.push(structuredClone(port))
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    zone.connections = zone.connections.filter(edge => edgeEndpoints(edge).join('→') !== 'low_flow_condition.y→low_flow_delay.u')
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!alarmBlocks.has(to.split('.')[0]) && to !== 'low_airflow_alarm_level' &&
          !(from === 'low_flow_occupied_gate.y' && to === 'low_flow_delay.u')) continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    const point = `${name}_low_airflow_alarm_level`
    const output = reference.outputs?.find(port => port.name === point)
    const edge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === point)
    if (!output || !edge) throw new Error(`G36 airflow alarm migration requires ${point}`)
    if (!next.graph.outputs.some(port => port.name === point)) next.graph.outputs.push(structuredClone(output))
    if (!next.graph.connections.some(current => edgeEndpoints(current)[1] === point))
      next.graph.connections.push(structuredClone(edge))
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
