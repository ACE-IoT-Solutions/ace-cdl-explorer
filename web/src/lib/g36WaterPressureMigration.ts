import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const loops = [
  { loop: 'hot', water: 'hot_water', reset: 'hot_reset_previous' },
  { loop: 'chilled', water: 'chilled_water', reset: 'chilled_reset_previous' },
] as const

/** Add measured remote water-pressure control while keeping the saved plant. */
export function migrateG36WaterPressure(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previousPlant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (loops.every(({ loop }) => previousPlant?.blocks?.some(block => block.name === `${loop}_dp_pi`))) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 water-pressure migration requires plant')
  const addPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 water-pressure reference lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const { loop, water } of loops) {
    const sensor = `${water}_differential_pressure_pa`
    const setpoint = `${loop}_dp_setpoint_pa`
    addPort(next.graph, reference, 'inputs', sensor)
    addPort(plant, template, 'inputs', sensor)
    addPort(plant, template, 'outputs', setpoint)
    addPort(next.graph, reference, 'outputs', `plant_${setpoint}`)
    const oldMinimum = plant.blocks?.find(block => block.name === `${water}_pump_limit`)
      ?.parameters?.find(parameter => parameter.name === 'uMin')?.value
    plant.blocks = plant.blocks?.filter(block => ![`${water}_pump_gain`, `${water}_pump_limit`].includes(block.name))
    plant.connections = plant.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return ![`${water}_pump_gain`, `${water}_pump_limit`].some(name => from.startsWith(`${name}.`) || to.startsWith(`${name}.`))
    })
    for (const child of template.blocks ?? []) {
      if (!child.name.startsWith(`${loop}_dp_`) || plant.blocks?.some(block => block.name === child.name)) continue
      const added = structuredClone(child)
      if (added.name === `${loop}_dp_pump_minimum` && typeof oldMinimum === 'number') {
        const parameter = added.parameters?.find(parameter => parameter.name === 'k')
        if (parameter) parameter.value = oldMinimum
      }
      (plant.blocks ??= []).push(added)
    }
    plant.connections = (plant.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(to === `${water}_pump_select.u1` && from !== `${loop}_dp_speed_minimum.y`)
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from.startsWith(`${loop}_dp_`) || to.startsWith(`${loop}_dp_`)
        || from === sensor || to === setpoint) addEdge(plant, from, to)
    }
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === sensor || to === `plant_${setpoint}`) addEdge(next.graph, from, to)
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
