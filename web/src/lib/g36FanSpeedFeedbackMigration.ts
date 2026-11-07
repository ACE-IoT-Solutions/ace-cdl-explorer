import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36FanSpeedFeedbackMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'fan_speed_tracking_select'))
}

/** Add physical drive feedback and a CDL diagnostic while preserving edited controls. */
export function migrateG36FanSpeedFeedback(saved: Project, reference: Block): Project {
  if (!needsG36FanSpeedFeedbackMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const rootEdges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 fan speed feedback migration requires ${name}`)
    ahu.inputs ??= []
    ahu.outputs ??= []
    ahu.blocks ??= []
    ahu.connections ??= []
    const pointName = `${name}_actual_fan_speed`
    const rootInput = reference.inputs?.find(port => port.name === pointName)
    const innerInput = template.inputs?.find(port => port.name === 'actual_fan_speed')
    const rootOutputName = `${name}_fan_speed_tracking_alarm_level`
    const rootOutput = reference.outputs?.find(port => port.name === rootOutputName)
    const innerOutput = template.outputs?.find(port => port.name === 'fan_speed_tracking_alarm_level')
    if (!rootInput || !innerInput || !rootOutput || !innerOutput)
      throw new Error(`G36 fan speed feedback migration missing ${name} ports`)
    if (!next.graph.inputs.some(port => port.name === pointName)) next.graph.inputs.push(structuredClone(rootInput))
    if (!ahu.inputs.some(port => port.name === 'actual_fan_speed')) ahu.inputs.push(structuredClone(innerInput))
    if (!next.graph.outputs.some(port => port.name === rootOutputName)) next.graph.outputs.push(structuredClone(rootOutput))
    if (!ahu.outputs.some(port => port.name === 'fan_speed_tracking_alarm_level')) ahu.outputs.push(structuredClone(innerOutput))
    for (const block of template.blocks ?? []) {
      if (block.name.startsWith('fan_speed_tracking_') && !ahu.blocks.some(current => current.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    const innerEdges = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!to.startsWith('fan_speed_tracking_')) continue
      const key = `${from}→${to}`
      if (!innerEdges.has(key)) { ahu.connections.push(structuredClone(edge)); innerEdges.add(key) }
    }
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to !== `${name}.actual_fan_speed` && to !== rootOutputName) continue
      const key = `${from}→${to}`
      if (!rootEdges.has(key)) { next.graph.connections.push(structuredClone(edge)); rootEdges.add(key) }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
