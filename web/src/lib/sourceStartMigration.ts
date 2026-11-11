import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const equipment = ['heat_pump', 'boiler', 'chiller'] as const
const startBlocks = new Set([
  'source_start_threshold', 'heat_pump_heat_start_positive',
  'heat_pump_cool_start_positive', 'heat_pump_start_or',
  'boiler_start_positive', 'chiller_start_positive',
])
const hasTarget = (graph: Block, target: string) =>
  graph.connections?.some(edge => edgeEndpoints(edge)[1] === target) ?? false

export function needsSourceStartMigration(saved: Project): boolean {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant' && block.block_type === 'PlantDispatch')
  if (!plant || !saved.graph.blocks?.some(block => block.block_type === 'AHUController')) return false
  return equipment.some(source => {
    const port = `${source}_start_command`
    return !plant.outputs?.some(output => output.name === port) ||
      !saved.graph.outputs?.some(output => output.name === `plant_${port}`) ||
      !hasTarget(plant, port) || !hasTarget(saved.graph, `plant_${port}`)
  }) || [...startBlocks].some(name => !plant.blocks?.some(block => block.name === name)) ||
    ['heat_pump_heat', 'heat_pump_cool', 'boiler', 'chiller'].some(name =>
      !hasTarget(plant, `${name}_start_positive.u1`) ||
      !hasTarget(plant, `${name}_start_positive.u2`))
}

/** Add explicit CDL source starts to an older building without replacing tuned blocks. */
export function migrateSourceStartRequests(saved: Project, reference: Block): Project {
  if (!needsSourceStartMigration(saved)) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('Source-start migration requires a plant reference')
  plant.blocks ??= []
  for (const name of startBlocks) {
    if (!plant.blocks.some(block => block.name === name)) {
      const source = template.blocks?.find(block => block.name === name)
      if (!source) throw new Error(`Source-start reference is missing ${name}`)
      plant.blocks.push(structuredClone(source))
    }
  }
  plant.outputs ??= []
  next.graph.outputs ??= []
  for (const source of equipment) {
    const port = `${source}_start_command`
    if (!plant.outputs.some(output => output.name === port)) {
      plant.outputs.push(structuredClone(template.outputs!.find(output => output.name === port)!))
    }
    if (!next.graph.outputs.some(output => output.name === `plant_${port}`)) {
      next.graph.outputs.push(structuredClone(reference.outputs!.find(output => output.name === `plant_${port}`)!))
    }
  }
  plant.connections ??= []
  next.graph.connections ??= []
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if ((startBlocks.has(to.split('.')[0]) || equipment.some(source => to === `${source}_start_command`)) &&
        !hasTarget(plant, to)) {
      const output = ({
        'heat_pump_heat_start_positive.u1': 'heat_pump_heating_kw',
        'heat_pump_cool_start_positive.u1': 'heat_pump_cooling_kw',
        'boiler_start_positive.u1': 'boiler_kw',
        'chiller_start_positive.u1': 'chiller_kw',
      } as Record<string,string>)[to]
      const actualSource = output
        ? plant.connections.find(connection => edgeEndpoints(connection)[1] === output)
        : undefined
      const directStart = ({
        heat_pump_start_command: 'heat_pump_start_or.y',
        boiler_start_command: 'boiler_start_positive.y',
        chiller_start_command: 'chiller_start_positive.y',
      } as Record<string,string>)[to]
      plant.connections.push({from: actualSource ? edgeEndpoints(actualSource)[0] : directStart ?? from, to})
    }
  }
  for (const source of equipment) {
    const target = `plant_${source}_start_command`
    if (!hasTarget(next.graph, target)) next.graph.connections.push({from:`plant.${source}_start_command`,to:target})
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
