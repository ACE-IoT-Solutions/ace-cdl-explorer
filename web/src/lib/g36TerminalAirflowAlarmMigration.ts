import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const alarmBlocks = new Set([
  'low_flow_condition', 'low_flow_delay',
  'low_flow_fan_ready', 'low_flow_occupied_and_fan',
  'low_flow_seventy_setpoint', 'low_flow_below_seventy', 'low_flow_eligible',
  'low_flow_occupied_gate', 'low_flow_seventy_condition', 'low_flow_seventy_delay',
  'low_flow_severe_fan_condition', 'low_flow_seventy_output_gate',
  'low_flow_severe_request_gate', 'low_flow_request_delay',
  'low_flow_alarm_zero', 'low_flow_alarm_three', 'low_flow_alarm_two',
  'low_flow_alarm_moderate', 'low_flow_alarm_priority',
])

export function needsG36TerminalAirflowAlarmMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
      .some(name => {
        const zone = saved.graph.blocks?.find(block => block.name === name)
        return zone && (!zone.blocks?.some(block => block.name === 'low_flow_alarm_priority') ||
          !zone.blocks?.some(block => block.name === 'low_flow_fan_ready') ||
          !zone.blocks?.some(block => block.name === 'low_flow_seventy_output_gate') ||
          !zone.blocks?.some(block => block.name === 'low_flow_request_delay') ||
          zone.blocks?.some(block => block.name === 'low_flow_severe_fan_condition'))
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
    zone.inputs ??= []
    zone.outputs ??= []
    zone.connections ??= []
    zone.blocks = zone.blocks.filter(block => block.name !== 'low_flow_severe_fan_condition')
    const fanPort = template.inputs?.find(port => port.name === 'supply_fan_proven')
    if (!fanPort) throw new Error(`G36 airflow alarm migration requires ${name} fan proof`)
    if (!zone.inputs.some(port => port.name === 'supply_fan_proven')) zone.inputs.push(structuredClone(fanPort))
    for (const block of template.blocks ?? []) {
      if (alarmBlocks.has(block.name) && !zone.blocks.some(existing => existing.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    // OBC TrueDelay does not delay an already running fan at simulation start.
    // Preserve the user's configured startup delay while correcting that state rule.
    const fanReady = zone.blocks.find(block => block.name === 'low_flow_fan_ready')
    const delayOnInit = fanReady?.parameters?.find(parameter => parameter.name === 'delayOnInit')
    if (delayOnInit) delayOnInit.value = false
    const port = template.outputs?.find(port => port.name === 'low_airflow_alarm_level')
    if (!port) throw new Error(`G36 airflow alarm migration requires ${name} alarm output`)
    if (!zone.outputs.some(existing => existing.name === port.name)) zone.outputs.push(structuredClone(port))
    zone.connections = zone.connections.filter(edge => {
      const [, to] = edgeEndpoints(edge)
      return !alarmBlocks.has(to.split('.')[0]) &&
        to !== 'low_airflow_alarm_level' && to !== 'low_airflow_alarm'
    })
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    const hasImportanceGate = zone.blocks.some(block => block.name === 'low_flow_importance_gate')
    for (const edge of template.connections ?? []) {
      let [from, to] = edgeEndpoints(edge)
      if (from === 'low_flow_importance_gate.y' && !hasImportanceGate &&
          (to === 'low_flow_eligible.u1' || to === 'low_flow_occupied_gate.u1'))
        from = 'low_flow_occupied_and_fan.y'
      if (!alarmBlocks.has(to.split('.')[0]) && to !== 'low_airflow_alarm_level' &&
          to !== 'low_airflow_alarm' &&
          !(hasImportanceGate && to === 'low_flow_importance_gate.u1')) continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push({ from, to })
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
    const fanEdge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === `${name}.supply_fan_proven`)
    if (!fanEdge) throw new Error(`G36 airflow alarm migration requires ${name} fan proof wiring`)
    if (!next.graph.connections.some(current => edgeEndpoints(current)[1] === `${name}.supply_fan_proven`))
      next.graph.connections.push(structuredClone(fanEdge))
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}

const hystereticComparators = ['flow_requested', 'low_flow_detected', 'low_flow_below_seventy']

export function needsG36LowFlowHysteresisMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(zone => zone.block_type === 'VAVBoxController').some(zone =>
      hystereticComparators.some(name => {
        const block = zone.blocks?.find(child => child.name === name)
        return block && !block.parameters?.some(parameter => parameter.name === 'h')
      }))
}

/** Expose OBC comparator hysteresis while keeping all saved thresholds and control wiring. */
export function migrateG36LowFlowHysteresis(saved: Project, reference: Block): Project {
  if (!needsG36LowFlowHysteresisMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(zone => zone.block_type === 'VAVBoxController')) {
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`G36 low-flow hysteresis migration requires ${zone.name}`)
    for (const name of hystereticComparators) {
      const block = zone.blocks?.find(child => child.name === name)
      if (!block || block.parameters?.some(parameter => parameter.name === 'h')) continue
      const source = template.blocks?.find(child => child.name === name)
      const h = source?.parameters?.find(parameter => parameter.name === 'h')
      if (!h) throw new Error(`G36 low-flow hysteresis migration requires ${zone.name}.${name}.h`)
      block.parameters ??= []
      block.parameters.push(structuredClone(h))
    }
  }
  return next
}
