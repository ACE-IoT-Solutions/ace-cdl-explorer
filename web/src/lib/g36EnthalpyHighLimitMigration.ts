import { edgeEndpoints, layoutFor, type Block, type Connection, type Project } from './types'

const point = 'outdoor_air_enthalpy_kj_kg'
const added = new Set([
  'economizer_enthalpy_cutoff', 'economizer_enthalpy_error',
  'economizer_enthalpy_hysteresis', 'economizer_enthalpy_and_dry_bulb_trip',
  'economizer_use_enthalpy_device', 'economizer_high_limit_device_select',
])
const targets = ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
  'economizer_high_limit_clear.u']

export function needsG36EnthalpyHighLimitMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (!saved.graph.inputs?.some(port => port.name === point) || [1, 2].some(index =>
      !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
        ?.blocks?.some(block => block.name === 'economizer_high_limit_device_select')))
}

/** Add an optional CDL device without changing the previously selected high limit. */
export function migrateG36EnthalpyHighLimit(saved: Project, reference: Block): Project {
  if (!needsG36EnthalpyHighLimitMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.connections ??= []
  const input = reference.inputs?.find(port => port.name === point)
  if (!input) throw new Error('Reference G36 graph lacks outdoor enthalpy input')
  if (!next.graph.inputs.some(port => port.name === point)) next.graph.inputs.push(structuredClone(input))
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 enthalpy migration requires ${name}`)
    ahu.inputs ??= []; ahu.blocks ??= []; ahu.connections ??= []
    const ahuInput = template.inputs?.find(port => port.name === point)
    if (!ahuInput) throw new Error(`Reference ${name} lacks outdoor enthalpy input`)
    if (!ahu.inputs.some(port => port.name === point)) ahu.inputs.push(structuredClone(ahuInput))
    if (!next.graph.connections.some(edge => edgeEndpoints(edge).join('→') === `${point}→${name}.${point}`))
      next.graph.connections.push({ from: point, to: `${name}.${point}` })
    if (ahu.blocks.some(block => block.name === 'economizer_high_limit_device_select')) continue
    const prior = new Map<string, string>()
    for (const target of targets) {
      const wires: Connection[] = ahu.connections.filter((edge: Connection) => edgeEndpoints(edge)[1] === target)
      if (wires.length !== 1) throw new Error(`G36 enthalpy migration requires one ${name}.${target} driver`)
      prior.set(target, edgeEndpoints(wires[0])[0])
      ahu.connections = ahu.connections.filter((edge: Connection) => edge !== wires[0])
    }
    const previousDevice = prior.get(targets[0])!
    // A custom high-limit source remains the default input to the new switch.
    for (const block of template.blocks ?? [])
      if (added.has(block.name)) ahu.blocks.push(structuredClone(block))
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'economizer_high_limit_device_select.u3' || targets.includes(to)) continue
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) && from !== point) continue
      const key = `${from}→${to}`
      if (!present.has(key)) { ahu.connections.push(structuredClone(edge)); present.add(key) }
    }
    ahu.connections.push({ from: previousDevice, to: 'economizer_high_limit_device_select.u3' })
    for (const target of targets) {
      const earlierSource = prior.get(target)!
      if (earlierSource === previousDevice) {
        ahu.connections.push({ from: 'economizer_high_limit_device_select.y', to: target })
        continue
      }
      // An edited direct-return qualification can differ from the economizer
      // hold input. Preserve that source whenever the new device is off.
      const selection = structuredClone(template.blocks!.find(block =>
        block.name === 'economizer_high_limit_device_select')!)
      selection.name = `${target.split('.')[0]}_enthalpy_select`
      ahu.blocks.push(selection)
      ahu.connections.push(
        { from: 'economizer_enthalpy_and_dry_bulb_trip.y', to: `${selection.name}.u1` },
        { from: 'economizer_use_enthalpy_device.y', to: `${selection.name}.u2` },
        { from: earlierSource, to: `${selection.name}.u3` },
        { from: `${selection.name}.y`, to: target },
      )
    }
  }
  if (reference.brick_model && next.graph.brick_model) {
    const entity = reference.brick_model.entities.find(entity => entity.id === point)
    const relation = reference.brick_model.relationships.find(relation => relation.object === point)
    if (entity && !next.graph.brick_model.entities.some(item => item.id === point))
      next.graph.brick_model.entities.push(structuredClone(entity))
    if (relation && !next.graph.brick_model.relationships.some(item => item.object === point))
      next.graph.brick_model.relationships.push(structuredClone(relation))
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
