import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const names = new Set([
  'fan_alarm_command_threshold', 'fan_alarm_commanded_on', 'fan_alarm_status_off',
  'fan_alarm_commanded_off', 'fan_alarm_failed_start', 'fan_alarm_failed_stop',
  'fan_alarm_start_delay', 'fan_alarm_stop_delay', 'fan_alarm_clear',
  'fan_alarm_level_two', 'fan_alarm_level_four', 'fan_alarm_stop_select',
  'fan_alarm_priority',
])

export function needsG36SupplyFanAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index => {
    const ahu = saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    return ahu && !ahu.blocks?.some(block => block.name === 'fan_alarm_priority')
  })
}

export function migrateG36SupplyFanAlarms(saved: Project, reference: Block): Project {
  if (!needsG36SupplyFanAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.outputs ??= []
  next.graph.connections ??= []
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 fan alarm migration requires ${name}`)
    ahu.blocks ??= []
    ahu.outputs ??= []
    ahu.connections ??= []
    for (const block of template.blocks ?? []) {
      if (names.has(block.name) && !ahu.blocks.some(existing => existing.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    const alarm = template.outputs?.find(port => port.name === 'supply_fan_alarm_level')
    if (!alarm) throw new Error(`G36 fan alarm migration requires ${name} output`)
    if (!ahu.outputs.some(port => port.name === alarm.name)) ahu.outputs.push(structuredClone(alarm))
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(to.split('.')[0]) && to !== 'supply_fan_alarm_level') continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        ahu.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    const point = `${name}_supply_fan_alarm_level`
    const output = reference.outputs?.find(port => port.name === point)
    const edge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === point)
    if (!output || !edge) throw new Error(`G36 fan alarm migration requires ${point}`)
    if (!next.graph.outputs.some(port => port.name === point)) next.graph.outputs.push(structuredClone(output))
    if (!next.graph.connections.some(current => edgeEndpoints(current)[1] === point))
      next.graph.connections.push(structuredClone(edge))
    const status = next.graph.inputs?.find(port => port.name === `${name}_supply_fan_proven`)
    if (status) {
      status.brick_class = 'Fan_Status'
      status.description = 'Measured fan motor running status from the previous physical step, independent of terminal airflow'
    }
    const innerStatus = ahu.inputs?.find(port => port.name === 'supply_fan_proven')
    if (innerStatus) innerStatus.brick_class = 'Fan_Status'
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
