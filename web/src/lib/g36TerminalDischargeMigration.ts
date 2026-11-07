import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const dischargeBlocks = new Set([
  'discharge_max_rise', 'discharge_upper', 'discharge_span', 'discharge_zero_rise',
  'discharge_positive_span', 'discharge_demand_double', 'discharge_demand_limit',
  'discharge_reset_rise', 'discharge_reset_target', 'discharge_target_cap',
  'discharge_minimum', 'discharge_target', 'discharge_valve_pi',
  'discharge_below_minimum', 'discharge_valve_enable',
  'discharge_valve_closed', 'discharge_valve_select',
  'heating_airflow_midpoint', 'heating_airflow_excess', 'heating_airflow_double',
  'heating_airflow_limit', 'heating_airflow_margin', 'heating_airflow_threshold',
  'heating_airflow_warm', 'heating_airflow_enabled', 'heating_airflow_zero',
  'heating_airflow_select', 'heating_warmup_full', 'heating_warmup_airflow',
])

/** Add measured-discharge reheat control while preserving saved zone tuning. */
export function migrateG36TerminalDischarge(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController')
    .every(zone => zone.blocks?.some(block => block.name === 'discharge_valve_pi')
      && zone.blocks?.some(block => block.name === 'heating_warmup_airflow'))) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, source: Block, direction: 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => { const [a, b] = edgeEndpoints(edge); return a === from && b === to })) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 discharge migration requires ${name}`)
    addPort(zone, template, 'outputs', 'discharge_air_setpoint_c')
    addPort(graph, reference, 'outputs', `${name}_discharge_air_setpoint_c`)
    for (const child of template.blocks ?? []) {
      if (dischargeBlocks.has(child.name) && !zone.blocks?.some(block => block.name === child.name)) {
        (zone.blocks ??= []).push(structuredClone(child))
      }
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(from === 'reheat_select.y' && to === 'reheat_capped.u1')
        && !(from === 'heat_pi.y' && to === 'heating_damper_gain.u')
        && !(from === 'heating_airflow_select.y' && to === 'heating_damper_gain.u')
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!dischargeBlocks.has(from.split('.')[0]) && !dischargeBlocks.has(to.split('.')[0]) && to !== 'discharge_air_setpoint_c') continue
      if (to === 'reheat_capped.u1' && zone.connections.some(existing => edgeEndpoints(existing)[1] === to)) continue
      if (to === 'heating_damper_gain.u' && zone.connections.some(existing => edgeEndpoints(existing)[1] === to)) continue
      addEdge(zone, from, to)
    }
    addEdge(graph, `${name}.discharge_air_setpoint_c`, `${name}_discharge_air_setpoint_c`)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
