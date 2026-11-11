import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sourceDrivers = {
  heat_pump: 'heat_pump_start_or.y',
  boiler: 'boiler_start_positive.y',
  chiller: 'chiller_start_positive.y',
} as const
const priorFaultDrivers = {
  heat_pump: 'heat_pump_start_request.y',
  boiler: 'boiler_availability_gate.y',
  chiller: 'chiller_availability_gate.y',
} as const

const outputDriver = (plant: Block, source: string) =>
  plant.connections?.find(edge => edgeEndpoints(edge)[1] === `${source}_start_command`)

function g36Plant(project: Project): Block | undefined {
  if (!project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return
  return project.graph.blocks.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
}

export function needsG36SourceMinRunMigration(project: Project): boolean {
  const plant = g36Plant(project)
  return !!plant && Object.entries(sourceDrivers).some(([source, driver]) => {
    const output = outputDriver(plant,source)
    const from = output && edgeEndpoints(output)[0]
    const observed = plant.connections?.find(edge => edgeEndpoints(edge)[1] === `${source}_start_request_observed.u`)
    const observedFrom = observed && edgeEndpoints(observed)[0]
    return from === driver || (from === `${source}_start_gate.y` &&
      (!plant.blocks?.some(block => block.name === `${source}_issued_start_real`) ||
        !observedFrom || observedFrom === priorFaultDrivers[source as keyof typeof priorFaultDrivers]))
  })
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
    const outputEdge = outputDriver(plant,source)
    const from = outputEdge && edgeEndpoints(outputEdge)[0]
    if (from !== driver && from !== `${source}_start_gate.y`) continue
    const wasDirect = from === driver
    const hadHold = plant.blocks.some(block => block.name === `${source}_minimum_run`)
    const added = new Set([
      `${source}_minimum_run`, `${source}_start_fault_clear`,
      `${source}_start_allowed`, `${source}_start_gate`, `${source}_issued_start_real`,
    ])
    for (const name of added) {
      if (plant.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 source minimum-run reference is missing ${name}`)
      plant.blocks.push(structuredClone(block))
    }
    const observed = `${source}_start_request_observed.u`
    const currentFaultDriver = plant.connections.find(edge => edgeEndpoints(edge)[1] === observed)
    const oldFaultDriver = priorFaultDrivers[source as keyof typeof priorFaultDrivers]
    const canUseIssuedFeedback = !currentFaultDriver || [oldFaultDriver, `${source}_issued_start_real.y`]
      .includes(edgeEndpoints(currentFaultDriver)[0])
    const hold = plant.blocks.find(block => block.name === `${source}_minimum_run`)!
    const offDuration = hold.parameters?.find(parameter => parameter.name === 'falseHoldDuration')
    if (offDuration?.value === 0 && canUseIssuedFeedback) offDuration.value = 300
    if (!hadHold && !canUseIssuedFeedback && offDuration?.value === 300) offDuration.value = 0
    if (wasDirect) plant.connections = plant.connections.filter(edge => edgeEndpoints(edge)[1] !== output)
    if (canUseIssuedFeedback && (!currentFaultDriver || edgeEndpoints(currentFaultDriver)[0] !== `${source}_issued_start_real.y`)) {
      plant.connections = plant.connections.filter(edge => edgeEndpoints(edge)[1] !== observed)
      const observedBlock = plant.blocks.find(block => block.name === `${source}_start_request_observed`)
      const templateObserved = template.blocks?.find(block => block.name === `${source}_start_request_observed`)
      if (observedBlock && templateObserved) observedBlock.notes = templateObserved.notes
    }
    for (const edge of template.connections ?? []) {
      const [, to] = edgeEndpoints(edge)
      if (!added.has(to.split('.')[0]) && to !== output && to !== observed) continue
      if (!plant.connections.some(existing => edgeEndpoints(existing)[1] === to)) {
        plant.connections.push(to === output || to === `${source}_issued_start_real.u`
          ? {from:`${source}_start_gate.y`,to}
          : to === `${source}_minimum_run.u` ? {from:driver,to}
          : structuredClone(edge))
      }
    }
  }
  next.layout = {...layoutFor(next.graph), ...project.layout}
  return next
}
