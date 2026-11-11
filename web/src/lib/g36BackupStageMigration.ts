import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sources = ['boiler', 'chiller'] as const

function plantFor(project: Project): Block | undefined {
  if (!project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return
  return project.graph.blocks.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
}

function inputDriver(plant: Block, target: string): string | undefined {
  const edge = plant.connections?.find(edge => edgeEndpoints(edge)[1] === target)
  return edge && edgeEndpoints(edge)[0]
}

export function needsG36BackupStageMigration(project: Project): boolean {
  const plant = plantFor(project)
  if (!plant) return false
  return sources.some(source => {
    const delay = `${source}_stage_up_delay`
    const output = inputDriver(plant, `${source}_start_command`)
    if (output !== `${source}_restart_gate.y` && output !== `${source}_start_gate.y`) return false
    const driver = inputDriver(plant, `${source}_minimum_run.u`)
    if (driver !== `${source}_start_positive.y` && driver !== `${delay}.y`) return false
    return driver === `${source}_start_positive.y` ||
      !plant.blocks?.some(block => block.name === delay) ||
      inputDriver(plant, `${delay}.u`) === undefined
  })
}

/** Qualify stock backup starts in editable CDL while retaining custom wiring and tuning. */
export function migrateG36BackupStage(project: Project, reference: Block): Project {
  if (!needsG36BackupStageMigration(project)) return project
  const next = structuredClone(project)
  const plant = plantFor(next)!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 backup-stage migration requires a plant reference')
  plant.blocks ??= []
  plant.connections ??= []
  for (const source of sources) {
    const delay = `${source}_stage_up_delay`
    const output = inputDriver(plant, `${source}_start_command`)
    if (output !== `${source}_restart_gate.y` && output !== `${source}_start_gate.y`) continue
    const target = `${source}_minimum_run.u`
    const driver = inputDriver(plant, target)
    if (driver !== `${source}_start_positive.y` && driver !== `${delay}.y`) continue
    if (!plant.blocks.some(block => block.name === delay)) {
      const block = template.blocks?.find(block => block.name === delay)
      if (!block) throw new Error(`G36 backup-stage reference is missing ${delay}`)
      plant.blocks.push(structuredClone(block))
    }
    // The physical-stop lockout now owns the backup source's off period. The
    // former stock 300 s hold would otherwise hide edits to a shorter stage delay.
    const offHold = plant.blocks.find(block => block.name === `${source}_minimum_run`)
      ?.parameters?.find(parameter => parameter.name === 'falseHoldDuration')
    if (offHold?.value === 300) offHold.value = 0
    if (driver === `${source}_start_positive.y`) {
      const edge = plant.connections.find(edge => edgeEndpoints(edge)[1] === target)!
      edge.from = `${delay}.y`
    }
    if (!inputDriver(plant, `${delay}.u`)) {
      const edge = template.connections?.find(edge => edgeEndpoints(edge)[1] === `${delay}.u`)
      if (!edge) throw new Error(`G36 backup-stage reference is missing ${delay}.u`)
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = {...layoutFor(next.graph), ...project.layout}
  return next
}
