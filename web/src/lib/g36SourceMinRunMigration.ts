import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sourceDrivers = {
  heat_pump: 'heat_pump_start_or.y',
  boiler: 'boiler_start_positive.y',
  chiller: 'chiller_start_positive.y',
} as const

function g36Plant(project: Project): Block | undefined {
  if (!project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return
  return project.graph.blocks.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
}

export function needsG36SourceMinRunMigration(project: Project): boolean {
  const plant = g36Plant(project)
  return !!plant && Object.entries(sourceDrivers).some(([source, driver]) =>
    plant.connections?.some(edge => {
      const [from, to] = edgeEndpoints(edge)
      return to === `${source}_start_command` && from === driver
    }))
}

/** Add a minimum source run period to stock start paths; preserve custom output wiring. */
export function migrateG36SourceMinRun(project: Project, reference: Block): Project {
  if (!needsG36SourceMinRunMigration(project)) return project
  const next = structuredClone(project)
  const plant = g36Plant(next)!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 source minimum-run migration requires a plant reference')
  plant.blocks ??= []
  plant.connections ??= []
  for (const [source, driver] of Object.entries(sourceDrivers)) {
    const output = `${source}_start_command`
    if (!plant.connections.some(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === driver && to === output
    })) continue
    const added = new Set([
      `${source}_minimum_run`, `${source}_start_fault_clear`,
      `${source}_start_allowed`, `${source}_start_gate`,
    ])
    for (const name of added) {
      if (plant.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 source minimum-run reference is missing ${name}`)
      plant.blocks.push(structuredClone(block))
    }
    plant.connections = plant.connections.filter(edge => edgeEndpoints(edge)[1] !== output)
    for (const edge of template.connections ?? []) {
      const [, to] = edgeEndpoints(edge)
      if (!added.has(to.split('.')[0]) && to !== output) continue
      if (!plant.connections.some(existing => edgeEndpoints(existing)[1] === to)) {
        plant.connections.push(structuredClone(edge))
      }
    }
  }
  next.layout = {...layoutFor(next.graph), ...project.layout}
  return next
}
