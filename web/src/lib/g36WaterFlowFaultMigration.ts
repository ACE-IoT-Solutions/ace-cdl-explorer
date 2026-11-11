import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add CDL diagnostics for a proven pump motor without measured loop flow. */
export function migrateG36WaterFlowFault(saved: Project, reference: Block): Project {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template || !plant.blocks?.some(block => block.name === 'hot_source_flow_present')) return saved
  const loops = ['hot', 'chilled'] as const
  if (loops.every(loop => saved.graph.outputs?.some(port => port.name === `plant_${loop}_water_flow_fault`) &&
    plant.outputs?.some(port => port.name === `${loop}_water_flow_fault`) &&
    plant.blocks?.some(block => block.name === `${loop}_flow_fault_delay`) &&
    plant.connections?.some(edge => edgeEndpoints(edge)[1] === `${loop}_water_flow_fault`) &&
    saved.graph.connections?.some(edge => edgeEndpoints(edge)[1] === `plant_${loop}_water_flow_fault`))) return saved

  const next = structuredClone(saved)
  const target = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const loop of loops) {
    const prefix = `${loop}_flow_fault_`
    const output = `${loop}_water_flow_fault`
    for (const child of template.blocks ?? []) {
      if (child.name.startsWith(prefix) && !target.blocks?.some(block => block.name === child.name)) {
        (target.blocks ??= []).push(structuredClone(child))
      }
    }
    const plantPort = template.outputs?.find(port => port.name === output)
    const rootPort = reference.outputs?.find(port => port.name === `plant_${output}`)
    if (!plantPort || !rootPort) throw new Error(`G36 flow-fault reference lacks ${output}`)
    if (!target.outputs?.some(port => port.name === output)) (target.outputs ??= []).push(structuredClone(plantPort))
    if (!next.graph.outputs?.some(port => port.name === `plant_${output}`)) (next.graph.outputs ??= []).push(structuredClone(rootPort))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!to.startsWith(prefix) && to !== output) continue
      if (!(target.connections ?? []).some(existing => edgeEndpoints(existing)[1] === to)) {
        (target.connections ??= []).push({ from, to })
      }
    }
    const rootTarget = `plant_${output}`
    if (!(next.graph.connections ?? []).some(edge => edgeEndpoints(edge)[1] === rootTarget)) {
      (next.graph.connections ??= []).push({ from: `plant.${output}`, to: rootTarget })
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
