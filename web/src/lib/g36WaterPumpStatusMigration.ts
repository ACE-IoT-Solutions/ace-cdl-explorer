import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add raw pump motor feedback to the editable flow proof without replacing tuned thresholds. */
export function migrateG36WaterPumpStatus(saved: Project, reference: Block): Project {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant?.blocks?.some(block => block.name === 'hot_source_proven') ||
      !plant.blocks.some(block => block.name === 'chilled_source_proven') || !template) return saved
  if (['hot', 'chilled'].every(loop =>
      plant.inputs?.some(port => port.name === `${loop}_water_pump_running`) &&
      saved.graph.inputs?.some(port => port.name === `${loop}_water_pump_running`))) return saved

  const next = structuredClone(saved)
  const target = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const loop of ['hot', 'chilled']) {
    const signal = `${loop}_water_pump_running`
    const blockName = `${loop}_source_motor_and_flow`
    const rootPort = reference.inputs?.find(port => port.name === signal)
    const plantPort = template.inputs?.find(port => port.name === signal)
    const block = template.blocks?.find(child => child.name === blockName)
    if (!rootPort || !plantPort || !block) throw new Error(`G36 pump-status reference lacks ${signal}`)
    if (!next.graph.inputs?.some(port => port.name === signal)) (next.graph.inputs ??= []).push(structuredClone(rootPort))
    if (!target.inputs?.some(port => port.name === signal)) (target.inputs ??= []).push(structuredClone(plantPort))
    const rootConnection = `${signal}→plant.${signal}`
    if (!(next.graph.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === rootConnection)) {
      (next.graph.connections ??= []).push({ from: signal, to: `plant.${signal}` })
    }
    const oldFrom = `${loop}_source_flow_present.y`
    const proofInput = `${loop}_source_proven.u2`
    const oldEdge = (target.connections ?? []).find(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === oldFrom && to === proofInput
    })
    // Respect a user-edited source for this proof input. The new raw point is
    // still available to wire in the editor if that path was customized.
    if (oldEdge) {
      if (!target.blocks?.some(child => child.name === blockName)) (target.blocks ??= []).push(structuredClone(block))
      target.connections = target.connections!.filter(edge => edge !== oldEdge)
      for (const [from, to] of [
        [oldFrom, `${blockName}.u1`],
        [signal, `${blockName}.u2`],
        [`${blockName}.y`, proofInput],
      ]) {
        if (!target.connections.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
          target.connections.push({ from, to })
        }
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
