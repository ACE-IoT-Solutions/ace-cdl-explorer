import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add editable pump start diagnostics from CDL command and raw motor status. */
export function migrateG36WaterPumpStartFault(saved: Project, reference: Block): Project {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template || !saved.graph.inputs?.some(port => port.name === 'hot_water_pump_running')) return saved
  const outputs = ['plant_hot_water_pump_start_fault', 'plant_chilled_water_pump_start_fault']
  if (outputs.every(name => saved.graph.outputs?.some(port => port.name === name))) return saved
  const next = structuredClone(saved)
  const target = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const loop of ['hot', 'chilled']) {
    const water = `${loop}_water`
    const prefix = `${loop}_pump_fault_`
    const output = `${water}_pump_start_fault`
    for (const child of template.blocks ?? []) {
      if (child.name.startsWith(prefix) && !target.blocks?.some(block => block.name === child.name)) {
        (target.blocks ??= []).push(structuredClone(child))
      }
    }
    const plantPort = template.outputs?.find(port => port.name === output)
    const rootPort = reference.outputs?.find(port => port.name === `plant_${output}`)
    if (!plantPort || !rootPort) throw new Error(`G36 pump-fault reference lacks ${output}`)
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
