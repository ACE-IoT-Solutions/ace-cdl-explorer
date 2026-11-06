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
