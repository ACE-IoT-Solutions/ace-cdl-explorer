import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const oldSuffixes = new Set([
  'sat_demand_threshold', 'sat_demand_high', 'sat_damper_open',
  'sat_request_condition', 'sat_request_delay', 'sat_request_count',
])
const zoneName = (index: number) => `zone_${String(index).padStart(2, '0')}`
const root = (endpoint: string) => endpoint.split('.')[0]

export function needsG36SatRequestLevelsMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return Array.from({ length: 10 }, (_, index) => zoneName(index + 1)).some(name =>
    !saved.graph.blocks?.find(block => block.name === name)?.outputs?.some(port =>
      port.name === 'cooling_sat_reset_request')) ||
    [1, 2].some(index => !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)?.inputs?.some(port =>
      port.name === `${zoneName((index - 1) * 5 + 1)}_sat_request`))
}

/** Move source-derived cooling requests into each editable VAV and retain AHU trim tuning. */
export function migrateG36SatRequestLevels(saved: Project, reference: Block): Project {
  if (!needsG36SatRequestLevelsMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.outputs ??= []
  next.graph.connections ??= []
  for (let index = 1; index <= 10; index++) {
    const name = zoneName(index)
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`SAT request migration requires ${name}`)
    zone.blocks ??= []
    zone.outputs ??= []
    zone.connections ??= []
    const newBlocks = (template.blocks ?? []).filter(block => block.name.startsWith('sat_'))
    const names = new Set(newBlocks.map(block => block.name))
    if (!names.has('sat_request_three')) throw new Error(`SAT request migration requires ${name} request branch`)
    for (const block of newBlocks) {
      if (!zone.blocks.some(existing => existing.name === block.name))
        zone.blocks.push(structuredClone(block))
    }
    const output = template.outputs?.find(port => port.name === 'cooling_sat_reset_request')
    if (!output) throw new Error(`SAT request migration requires ${name} output`)
    if (!zone.outputs.some(port => port.name === output.name)) zone.outputs.push(structuredClone(output))
    zone.connections = zone.connections.filter(edge => edgeEndpoints(edge)[1] !== 'cooling_sat_reset_request')
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(root(to)) && to !== 'cooling_sat_reset_request') continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    const point = `${name}_cooling_sat_reset_request`
    const rootOutput = reference.outputs?.find(port => port.name === point)
    const rootEdge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === point)
    if (!rootOutput || !rootEdge) throw new Error(`SAT request migration requires ${point}`)
    if (!next.graph.outputs.some(port => port.name === point)) next.graph.outputs.push(structuredClone(rootOutput))
    if (!next.graph.connections.some(edge => edgeEndpoints(edge)[1] === point))
      next.graph.connections.push(structuredClone(rootEdge))
    zone.notes = `${zone.notes ?? ''} Cooling SAT request levels and setpoint-change suppression are editable CDL.`.trim()
  }
  for (let ahuIndex = 1; ahuIndex <= 2; ahuIndex++) {
    const ahuName = `ahu_${ahuIndex}`
    const ahu = next.graph.blocks?.find(block => block.name === ahuName)
    const template = reference.blocks?.find(block => block.name === ahuName)
    if (!ahu || !template) throw new Error(`SAT request migration requires ${ahuName}`)
    ahu.blocks ??= []
    ahu.inputs ??= []
    ahu.connections ??= []
    for (let index = (ahuIndex - 1) * 5 + 1; index <= ahuIndex * 5; index++) {
      const name = zoneName(index)
      const former = new Set([...oldSuffixes].map(suffix => `${name}_${suffix}`))
      ahu.blocks = ahu.blocks.filter(block => !former.has(block.name))
      ahu.connections = ahu.connections.filter(edge => {
        const [from, to] = edgeEndpoints(edge)
        return !former.has(root(from)) && !former.has(root(to))
      })
      ahu.inputs = ahu.inputs.filter(port =>
        port.name !== `${name}_cooling_demand` && port.name !== `${name}_damper_position`)
      next.graph.connections = next.graph.connections.filter(edge =>
        ![`${ahuName}.${name}_cooling_demand`, `${ahuName}.${name}_damper_position`]
          .includes(edgeEndpoints(edge)[1]))
      const input = template.inputs?.find(port => port.name === `${name}_sat_request`)
      const rootEdge = reference.connections?.find(edge => edgeEndpoints(edge)[1] === `${ahuName}.${name}_sat_request`)
      const sumEdge = template.connections?.find(edge => edgeEndpoints(edge)[0] === `${name}_sat_request`)
      if (!input || !rootEdge || !sumEdge) throw new Error(`SAT request migration requires ${name} AHU wiring`)
      if (!ahu.inputs.some(port => port.name === input.name)) ahu.inputs.push(structuredClone(input))
      if (!next.graph.connections.some(edge => edgeEndpoints(edge)[1] === `${ahuName}.${name}_sat_request`))
        next.graph.connections.push(structuredClone(rootEdge))
      const sumTarget = edgeEndpoints(sumEdge)[1]
      ahu.connections = ahu.connections.filter(edge => edgeEndpoints(edge)[1] !== sumTarget)
      ahu.connections.push(structuredClone(sumEdge))
    }
    ahu.notes = `${ahu.notes ?? ''} Typed VAV requests enter the sampled SAT reset sum.`.trim()
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
