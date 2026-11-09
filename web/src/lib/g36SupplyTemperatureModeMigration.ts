import { edgeEndpoints, type Block, type Project } from './types'

const isModeBlock = (name: string): boolean => name.startsWith('sat_mode_')

export function needsG36SupplyTemperatureModeMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    [1, 2].some(index => !project.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'sat_mode_fan_select'))
}

/** Add group-mode SAT selection while retaining existing reset gains and layout. */
export function migrateG36SupplyTemperatureModes(saved: Project, reference: Block): Project {
  if (!needsG36SupplyTemperatureModeMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`Supply-temperature mode migration requires ${name}`)
    if (ahu.blocks?.some(block => block.name === 'sat_mode_fan_select')) continue
    const input = template.inputs?.find(port => port.name === 'operation_mode')
    if (!input) throw new Error(`Current G36 example lacks ${name}.operation_mode`)
    if (!ahu.inputs?.some(port => port.name === 'operation_mode'))
      (ahu.inputs ??= []).push(structuredClone(input))
    for (const child of template.blocks ?? []) {
      if (isModeBlock(child.name) && !ahu.blocks?.some(block => block.name === child.name))
        (ahu.blocks ??= []).push(structuredClone(child))
    }
    const downstream = new Set((template.connections ?? [])
      .filter(edge => edgeEndpoints(edge)[0] === 'sat_mode_fan_select.y')
      .map(edge => edgeEndpoints(edge)[1]))
    for (const edge of ahu.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'sat_output.y' && downstream.has(to)) edge.from = 'sat_mode_fan_select.y'
      if (from === 'heating_coil_needed.y' && to === 'freeze_stage_one_heating_enable.u1')
        edge.from = 'sat_mode_heating_permitted.y'
      if (from === 'cooling_coil_needed.y' && to === 'cooling_coil_enable.u3')
        edge.from = 'sat_mode_cooling_permitted.y'
    }
    const present = new Set((ahu.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!isModeBlock(from.split('.')[0]) && !isModeBlock(to.split('.')[0])) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { (ahu.connections ??= []).push(structuredClone(edge)); present.add(key) }
    }
    const connection = { from: `zone_group_${index}.operation_mode`, to: `${name}.operation_mode` }
    if (!(next.graph.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${connection.from}→${connection.to}`))
      (next.graph.connections ??= []).push(connection)
    ahu.notes = `${ahu.notes ?? ''} Group operation mode selects the supply-air target in editable CDL.`.trim()
  }
  return next
}
