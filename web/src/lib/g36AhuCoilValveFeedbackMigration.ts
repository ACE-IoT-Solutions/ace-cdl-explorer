import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

function isG36(graph: Block): boolean {
  return graph.name === 'FourPipeG36Controls' ||
    (graph.blocks ?? []).some(block => block.block_type === 'ZoneGroupModeController')
}

export function needsG36AhuCoilValveFeedbackMigration(saved: Project): boolean {
  return isG36(saved.graph) && [1, 2].some(index =>
    ['heating', 'cooling'].some(service =>
      !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
        ?.blocks?.some(block => block.name === `${service}_coil_valve_tracking_select`)))
}

/** Add raw valve-position inputs and CDL diagnostics while retaining edited AHU commands. */
export function migrateG36AhuCoilValveFeedback(saved: Project, reference: Block): Project {
  if (!needsG36AhuCoilValveFeedbackMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const rootEdges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 AHU coil valve feedback requires ${name}`)
    ahu.inputs ??= []
    ahu.outputs ??= []
    ahu.blocks ??= []
    ahu.connections ??= []
    const innerEdges = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const service of ['heating', 'cooling']) {
      const prefix = `${service}_coil_valve_tracking_`
      const sensor = `actual_${service}_coil_valve_position`
      const alarm = `${prefix}alarm_level`
      const rootSensor = `${name}_${sensor}`
      const rootAlarm = `${name}_${alarm}`
      for (const [collection, source, point] of [
        [next.graph.inputs, reference.inputs, rootSensor],
        [ahu.inputs, template.inputs, sensor],
        [next.graph.outputs, reference.outputs, rootAlarm],
        [ahu.outputs, template.outputs, alarm],
      ] as const) {
        const port = source?.find(item => item.name === point)
        if (!port) throw new Error(`G36 AHU coil valve feedback missing ${point}`)
        if (!collection.some(item => item.name === point)) collection.push(structuredClone(port))
      }
      for (const block of template.blocks ?? []) {
        if (block.name.startsWith(prefix) && !ahu.blocks.some(current => current.name === block.name))
          ahu.blocks.push(structuredClone(block))
      }
      const commandOutput = `${service}_coil_valve_position`
      const templateCommand = template.connections?.find(edge => edgeEndpoints(edge)[1] === commandOutput)
      const savedCommand = ahu.connections.find(edge => edgeEndpoints(edge)[1] === commandOutput)
      if (!templateCommand || !savedCommand) throw new Error(`G36 AHU coil valve command missing ${name}.${commandOutput}`)
      const templateSource = edgeEndpoints(templateCommand)[0]
      const savedSource = edgeEndpoints(savedCommand)[0]
      for (const edge of template.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (!to.startsWith(prefix) && to !== alarm) continue
        const actualFrom = from === templateSource ? savedSource : from
        const key = `${actualFrom}→${to}`
        if (!innerEdges.has(key)) {
          const copy = structuredClone(edge)
          copy.from = actualFrom
          ahu.connections.push(copy)
          innerEdges.add(key)
        }
      }
      for (const edge of reference.connections ?? []) {
        const [from, to] = edgeEndpoints(edge)
        if (to !== `${name}.${sensor}` && to !== rootAlarm) continue
        const key = `${from}→${to}`
        if (!rootEdges.has(key)) { next.graph.connections.push(structuredClone(edge)); rootEdges.add(key) }
      }
      if (next.graph.brick_model && reference.brick_model) {
        const entity = reference.brick_model.entities.find(item => item.id === rootSensor)
        if (entity && !next.graph.brick_model.entities.some(item => item.id === rootSensor))
          next.graph.brick_model.entities.push(structuredClone(entity))
        for (const relation of reference.brick_model.relationships.filter(item => item.object === rootSensor)) {
          if (!next.graph.brick_model.relationships.some(item =>
            item.subject === relation.subject && item.predicate === relation.predicate && item.object === relation.object))
            next.graph.brick_model.relationships.push(structuredClone(relation))
        }
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
