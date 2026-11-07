import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36AhuDamperFeedbackMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'return_damper_tracking_select'))
}

/** Add measured AHU dampers and CDL diagnostics without replacing tuned controls. */
export function migrateG36AhuDamperFeedback(saved: Project, reference: Block): Project {
  if (!needsG36AhuDamperFeedbackMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const rootEdges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 AHU damper feedback migration requires ${name}`)
    ahu.inputs ??= []
    ahu.outputs ??= []
    ahu.blocks ??= []
    ahu.connections ??= []
    const innerEdges = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const damper of ['outdoor', 'return']) {
      const prefix = `${damper}_damper_tracking_`
      const sensor = `actual_${damper}_damper_position`
      const alarm = `${damper}_damper_tracking_alarm_level`
      const rootSensor = `${name}_${sensor}`
      const rootAlarm = `${name}_${alarm}`
      for (const [collection, source, point] of [
        [next.graph.inputs, reference.inputs, rootSensor],
        [ahu.inputs, template.inputs, sensor],
        [next.graph.outputs, reference.outputs, rootAlarm],
        [ahu.outputs, template.outputs, alarm],
      ] as const) {
        const port = source?.find(item => item.name === point)
        if (!port) throw new Error(`G36 AHU damper feedback migration missing ${point}`)
        if (!collection.some(item => item.name === point)) collection.push(structuredClone(port))
      }
      for (const block of template.blocks ?? []) {
        if (block.name.startsWith(prefix) && !ahu.blocks.some(current => current.name === block.name))
          ahu.blocks.push(structuredClone(block))
      }
      for (const edge of template.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (!to.startsWith(prefix) && to !== alarm) continue
        const key = `${from}→${to}`
        if (!innerEdges.has(key)) { ahu.connections.push(structuredClone(edge)); innerEdges.add(key) }
      }
      for (const edge of reference.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (to !== `${name}.${sensor}` && to !== rootAlarm) continue
        const key = `${from}→${to}`
        if (!rootEdges.has(key)) { next.graph.connections.push(structuredClone(edge)); rootEdges.add(key) }
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
