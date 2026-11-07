import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const ahuInputs = [1, 2].flatMap(index => [
  `ahu_${index}_cooling_valve`, `ahu_${index}_supply_setpoint_c`, `ahu_${index}_supply_actual_c`,
])

/** Add the G36 chilled-water reset while retaining existing plant parameters and notes. */
export function migrateG36WaterReset(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.find(block => block.name === 'plant')?.blocks?.some(block => block.name === 'chilled_reset_previous')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const plant = graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 chilled-water reset migration requires the plant composite')
  const addPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => { const [a, b] = edgeEndpoints(edge); return a === from && b === to })) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const name of ahuInputs) addPort(plant, template, 'inputs', name)
  addPort(plant, template, 'outputs', 'chilled_water_reset_fraction')
  addPort(graph, reference, 'outputs', 'plant_chilled_water_reset_fraction')
  const newNames = new Set((template.blocks ?? []).map(block => block.name).filter(name =>
    name.startsWith('chilled_reset_') || /^ahu_[12]_(valve_request|supply_error|two_request|three_request|request_two|request_three|select_two_requests|select_three_requests)/.test(name)
  ))
  for (const child of template.blocks ?? []) {
    if (newNames.has(child.name) && !plant.blocks?.some(block => block.name === child.name)) {
      (plant.blocks ??= []).push(structuredClone(child))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(from === 'chilled_water_supply_setpoint.y' && to === 'chilled_water_supply_setpoint_c')
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (newNames.has(from.split('.')[0]) || newNames.has(to.split('.')[0]) || to === 'chilled_water_reset_fraction') {
      addEdge(plant, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (to.startsWith('plant.ahu_') || to === 'plant_chilled_water_reset_fraction') addEdge(graph, from, to)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
