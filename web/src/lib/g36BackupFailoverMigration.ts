import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sources = ['boiler','chiller'] as const
const shared = ['heat_pump_unavailable','heat_pump_failover'] as const
const sourceNames = (source:string) => [`${source}_failover_start`,`${source}_qualified_start`]

function plantFor(project:Project):Block|undefined {
  if (!project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return
  return project.graph.blocks.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
}
const driver = (plant:Block,target:string) => {
  const edge = plant.connections?.find(edge => edgeEndpoints(edge)[1] === target)
  return edge && edgeEndpoints(edge)[0]
}
const stockOutput = (plant:Block,source:string) =>
  [`${source}_restart_gate.y`,`${source}_start_gate.y`].includes(driver(plant,`${source}_start_command`) ?? '')

export function needsG36BackupFailoverMigration(project:Project):boolean {
  const plant = plantFor(project)
  if (!plant) return false
  return sources.some(source => {
    if (!stockOutput(plant,source)) return false
    const current = driver(plant,`${source}_minimum_run.u`)
    if (current !== `${source}_stage_up_delay.y` && current !== `${source}_qualified_start.y`) return false
    const names = [...shared,...sourceNames(source)]
    const targets = [
      'heat_pump_unavailable.u','heat_pump_failover.u1','heat_pump_failover.u2',
      `${source}_stage_up_delay.u`,
      `${source}_failover_start.u1`,`${source}_failover_start.u2`,
      `${source}_qualified_start.u1`,`${source}_qualified_start.u2`,
    ]
    return current === `${source}_stage_up_delay.y` ||
      names.some(name => !plant.blocks?.some(block => block.name === name)) ||
      targets.some(target => !driver(plant,target))
  })
}

/** Add a graph-owned fast failover for stock G36 backup stages. */
export function migrateG36BackupFailover(project:Project,reference:Block):Project {
  if (!needsG36BackupFailoverMigration(project)) return project
  const next = structuredClone(project)
  const plant = plantFor(next)!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 backup-failover migration requires a plant reference')
  plant.blocks ??= []
  plant.connections ??= []
  const eligible = sources.filter(source => stockOutput(plant,source) &&
    [`${source}_stage_up_delay.y`,`${source}_qualified_start.y`].includes(driver(plant,`${source}_minimum_run.u`) ?? ''))
  if (!eligible.length) return project
  const names = new Set([...shared,...eligible.flatMap(sourceNames)])
  for (const name of names) {
    if (plant.blocks.some(block => block.name === name)) continue
    const block = template.blocks?.find(block => block.name === name)
    if (!block) throw new Error(`G36 backup-failover reference is missing ${name}`)
    plant.blocks.push(structuredClone(block))
  }
  for (const source of eligible) {
    const target = `${source}_minimum_run.u`
    const edge = plant.connections.find(edge => edgeEndpoints(edge)[1] === target)!
    if (edgeEndpoints(edge)[0] === `${source}_stage_up_delay.y`) edge.from = `${source}_qualified_start.y`
  }
  const targets = new Set([
    'heat_pump_unavailable.u','heat_pump_failover.u1','heat_pump_failover.u2',
    ...eligible.flatMap(source => [
      `${source}_stage_up_delay.u`,
      `${source}_failover_start.u1`,`${source}_failover_start.u2`,
      `${source}_qualified_start.u1`,`${source}_qualified_start.u2`,
    ]),
  ])
  for (const edge of template.connections ?? []) {
    const [,to] = edgeEndpoints(edge)
    if (targets.has(to) && !driver(plant,to)) plant.connections.push(structuredClone(edge))
  }
  next.layout = {...layoutFor(next.graph),...project.layout}
  return next
}
