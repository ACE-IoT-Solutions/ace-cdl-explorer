import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sources = ['heat_pump','boiler','chiller'] as const
const newNames = (source:string) => [
  `${source}_stop_edge`, `${source}_forced_stop_lockout`,
  `${source}_forced_stop_clear`, `${source}_restart_gate`,
]
const hasTarget = (plant:Block, target:string) =>
  plant.connections?.some(edge => edgeEndpoints(edge)[1] === target) ?? false

function plantFor(project:Project):Block|undefined {
  if (!project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return
  return project.graph.blocks.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
}

export function needsG36ForcedStopMigration(project:Project):boolean {
  const plant = plantFor(project)
  if (!plant) return false
  return sources.some(source => {
    const output = plant.connections?.find(edge => edgeEndpoints(edge)[1] === `${source}_start_command`)
    const from = output && edgeEndpoints(output)[0]
    if (from !== `${source}_start_gate.y` && from !== `${source}_restart_gate.y`) return false
    return from === `${source}_start_gate.y` ||
      newNames(source).some(name => !plant.blocks?.some(block => block.name === name)) ||
      [...newNames(source).flatMap(name => name === `${source}_restart_gate` ? [`${name}.u1`,`${name}.u2`] : [`${name}.u`]),
        `${source}_issued_start_real.u`].some(target => !hasTarget(plant,target))
  })
}

/** Add a CDL restart lockout after physical running proof falls. */
export function migrateG36ForcedStop(project:Project, reference:Block):Project {
  if (!needsG36ForcedStopMigration(project)) return project
  const next = structuredClone(project)
  const plant = plantFor(next)!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 forced-stop migration requires a plant reference')
  plant.blocks ??= []
  plant.connections ??= []
  for (const source of sources) {
    const output = `${source}_start_command`
    const current = plant.connections.find(edge => edgeEndpoints(edge)[1] === output)
    const driver = current && edgeEndpoints(current)[0]
    if (driver !== `${source}_start_gate.y` && driver !== `${source}_restart_gate.y`) continue
    const added = new Set(newNames(source))
    for (const name of added) {
      if (plant.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 forced-stop reference is missing ${name}`)
      plant.blocks.push(structuredClone(block))
    }
    plant.connections = plant.connections.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return to !== output && !(to === `${source}_issued_start_real.u` && from === `${source}_start_gate.y`)
    })
    for (const edge of template.connections ?? []) {
      const [,to] = edgeEndpoints(edge)
      if (!added.has(to.split('.')[0]) && to !== output && to !== `${source}_issued_start_real.u`) continue
      if (!hasTarget(plant,to)) plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = {...layoutFor(next.graph),...project.layout}
  return next
}
