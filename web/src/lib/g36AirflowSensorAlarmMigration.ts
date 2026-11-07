import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const names = new Set([
  'sensor_fan_off', 'sensor_fan_off_delay', 'sensor_cooling_design_flow',
  'sensor_ten_percent', 'sensor_residual_flow', 'sensor_alarm_condition',
  'sensor_alarm_zero', 'sensor_alarm_three', 'sensor_alarm_select',
])

export function needsG36AirflowSensorAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => saved.graph.blocks?.find(block => block.name === name)
        ?.blocks?.every(block => block.name !== 'sensor_alarm_select'))
}

/** Add independent sensor diagnosis without replacing edited VAV controls. */
export function migrateG36AirflowSensorAlarms(saved: Project, reference: Block): Project {
  if (!needsG36AirflowSensorAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.outputs ??= []
  next.graph.connections ??= []
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 sensor alarm migration requires ${name}`)
    zone.blocks ??= []
    zone.outputs ??= []
    zone.connections ??= []
    for (const block of template.blocks ?? []) {
      if (names.has(block.name) && !zone.blocks.some(existing => existing.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    const alarm = template.outputs?.find(port => port.name === 'airflow_sensor_alarm_level')
    if (!alarm) throw new Error(`G36 sensor alarm migration requires ${name} output`)
    if (!zone.outputs.some(port => port.name === alarm.name)) zone.outputs.push(structuredClone(alarm))
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(to.split('.')[0]) && to !== 'airflow_sensor_alarm_level') continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    const point = `${name}_airflow_sensor_alarm_level`
    const output = reference.outputs?.find(port => port.name === point)
    const edge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === point)
    if (!output || !edge) throw new Error(`G36 sensor alarm migration requires ${point}`)
    if (!next.graph.outputs.some(port => port.name === point)) next.graph.outputs.push(structuredClone(output))
    if (!next.graph.connections.some(current => edgeEndpoints(current)[1] === point))
      next.graph.connections.push(structuredClone(edge))
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
