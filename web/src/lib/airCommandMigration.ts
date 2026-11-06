import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add graph-owned air commands introduced after older projects were saved. */
export function migrateOutdoorAirCommand(saved: Project, reference: Block): Project {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(saved.graph.name)) return saved
  const missing = [1, 2].filter(ahu => !saved.graph.outputs?.some(port => port.name === `ahu_${ahu}_outdoor_air_fraction`))
  const missingFlow = Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}`)
    .filter(name => !saved.graph.outputs?.some(port => port.name === `${name}_airflow_request`))
  if (!missing.length && !missingFlow.length) return saved
  const next = structuredClone(saved)
  for (const ahu of missing) {
    const name = `ahu_${ahu}`
    const controller = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const graphPort = reference.outputs?.find(port => port.name === `${name}_outdoor_air_fraction`)
    const controllerPort = template?.outputs?.find(port => port.name === 'outdoor_air_fraction')
    const constant = template?.blocks?.find(block => block.name === 'minimum_outdoor_air')
    if (!controller || !template || !graphPort || !controllerPort || !constant) throw new Error(`Cannot migrate ${name} outdoor-air command`)
    ;(next.graph.outputs ??= []).push(structuredClone(graphPort))
    if (!controller.outputs?.some(port => port.name === 'outdoor_air_fraction')) (controller.outputs ??= []).push(structuredClone(controllerPort))
    if (!controller.blocks?.some(block => block.name === constant.name)) (controller.blocks ??= []).push(structuredClone(constant))
    if (!(controller.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === 'minimum_outdoor_air.y|outdoor_air_fraction')) {
      ;(controller.connections ??= []).push({ from: 'minimum_outdoor_air.y', to: 'outdoor_air_fraction' })
    }
    if (!(next.graph.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${name}.outdoor_air_fraction|${name}_outdoor_air_fraction`)) {
      ;(next.graph.connections ??= []).push({ from: `${name}.outdoor_air_fraction`, to: `${name}_outdoor_air_fraction` })
    }
  }
  for (const name of missingFlow) {
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const output = reference.outputs?.find(port => port.name === `${name}_airflow_request`)
    const zonePort = template?.outputs?.find(port => port.name === 'airflow_request')
    const sum = template?.blocks?.find(block => block.name === 'airflow_request_sum')
    if (!zone || !template || !output || !zonePort || !sum || !zone.blocks?.some(block => block.name === 'minimum_airflow') || !zone.blocks?.some(block => block.name === 'flow_span')) {
      throw new Error(`Cannot migrate ${name} airflow request`)
    }
    ;(next.graph.outputs ??= []).push(structuredClone(output))
    if (!zone.outputs?.some(port => port.name === 'airflow_request')) (zone.outputs ??= []).push(structuredClone(zonePort))
    if (!zone.blocks.some(block => block.name === sum.name)) zone.blocks.push(structuredClone(sum))
    for (const [from, to] of [
      ['minimum_airflow.y', 'airflow_request_sum.u1'],
      ['flow_span.y', 'airflow_request_sum.u2'],
      ['airflow_request_sum.y', 'airflow_request'],
    ]) {
      if (!(zone.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${from}|${to}`)) {
        ;(zone.connections ??= []).push({ from, to })
      }
    }
    if (!(next.graph.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${name}.airflow_request|${name}_airflow_request`)) {
      ;(next.graph.connections ??= []).push({ from: `${name}.airflow_request`, to: `${name}_airflow_request` })
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}

/** Move AHU heating and cooling valve commands from implicit simulator behavior into CDL. */
export function migrateCoilControl(saved: Project, reference: Block): Project {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(saved.graph.name)) return saved
  if ([1, 2].every(ahu => saved.graph.outputs?.some(port => port.name === `ahu_${ahu}_heating_coil_valve_position`)
    && saved.graph.outputs?.some(port => port.name === `ahu_${ahu}_cooling_coil_valve_position`)
    && saved.graph.blocks?.find(block => block.name === `ahu_${ahu}`)?.blocks?.some(block => block.name === 'heating_coil_feedback')
    && saved.graph.blocks?.find(block => block.name === `ahu_${ahu}`)?.blocks?.some(block => block.name === 'cooling_coil_feedback'))) return saved
  const next = structuredClone(saved)
  const coilBlocks = new Set([
    'coil_zero', 'heating_coil_error', 'heating_coil_feedforward', 'heating_coil_feedback',
    'heating_coil_sum', 'heating_coil_limit', 'heating_coil_needed', 'heating_coil_enable',
    'cooling_coil_error', 'cooling_coil_feedforward', 'cooling_coil_feedback',
    'cooling_coil_sum', 'cooling_coil_limit', 'cooling_coil_needed', 'cooling_coil_enable',
  ])
  const copyPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current building example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const connect = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${from}|${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (let index = 1; index <= 2; index++) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`Coil control migration requires ${name}`)
    for (const input of ['actual_supply_air_temp', 'mixed_air_temp']) {
      copyPort(next.graph, reference, 'inputs', `${name}_${input}`)
      copyPort(ahu, template, 'inputs', input)
      connect(next.graph, `${name}_${input}`, `${name}.${input}`)
    }
    for (const output of ['heating_coil_valve_position', 'cooling_coil_valve_position']) {
      copyPort(next.graph, reference, 'outputs', `${name}_${output}`)
      copyPort(ahu, template, 'outputs', output)
      connect(next.graph, `${name}.${output}`, `${name}_${output}`)
    }
    for (const child of template.blocks ?? []) {
      if (coilBlocks.has(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (coilBlocks.has(from.split('.')[0]) || coilBlocks.has(to.split('.')[0])
        || ['heating_coil_valve_position', 'cooling_coil_valve_position'].includes(to)) connect(ahu, from, to)
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
