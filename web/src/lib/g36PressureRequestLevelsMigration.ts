import { edgeEndpoints, type Block, type Project } from './types'

const isZone = (block: Block) => block.block_type === 'VAVBoxController'
const root = (endpoint: string) => endpoint.split('.')[0]
const requestBlock = (name: string) =>
  (name.startsWith('pressure_') && name !== 'pressure_effect') ||
  name === 'damper_nearly_open' || name === 'low_flow_importance_gate'

export function needsG36PressureRequestLevelsMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(isZone).some(zone =>
      !zone.blocks?.some(block => block.name === 'pressure_damper_sample'))
}

/** Rebuild only the request branch; retain existing threshold and importance tuning. */
export function migrateG36PressureRequestLevels(saved: Project, reference: Block): Project {
  if (!needsG36PressureRequestLevelsMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(isZone)) {
    if (zone.blocks?.some(block => block.name === 'pressure_damper_sample')) continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`Pressure request migration requires ${zone.name}`)
    const oldBlocks = new Map((zone.blocks ?? []).map(block => [block.name, block]))
    zone.blocks = (zone.blocks ?? []).filter(block => !requestBlock(block.name))
    for (const source of template.blocks ?? []) {
      if (!requestBlock(source.name)) continue
      const block = structuredClone(source)
      const old = oldBlocks.get(block.name)
      if (old) {
        for (const parameter of block.parameters ?? []) {
          const previous = old.parameters?.find(item => item.name === parameter.name)
          if (previous) parameter.value = previous.value
        }
        if (old.notes) block.notes = old.notes
      }
      zone.blocks.push(block)
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !requestBlock(root(from)) && !requestBlock(root(to)) &&
        to !== 'pressure_reset_request' &&
        to !== 'low_flow_eligible.u1' && to !== 'low_flow_occupied_gate.u1'
    })
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!requestBlock(root(to)) && to !== 'pressure_reset_request' &&
          to !== 'low_flow_eligible.u1' && to !== 'low_flow_occupied_gate.u1') continue
      const key = `${from}→${to}`
      if (!present.has(key)) {
        zone.connections.push(structuredClone(edge))
        present.add(key)
      }
    }
    zone.notes = `${zone.notes ?? ''} Duct-pressure requests sample damper, setpoint, and measured flow together before the one-minute damper qualification.`.trim()
  }
  return next
}

/** Align stock terminal request thresholds with the pinned OBC SystemRequests source. */
export function needsG36PressureRequestHysteresisMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(isZone).some(zone => {
      const blocks = zone.blocks ?? []
      const positive = blocks.find(block => block.name === 'pressure_setpoint_positive')
      return positive?.block_type.endsWith('.Reals.Greater') && !(positive.parameters?.length) &&
        blocks.some(block => block.name === 'pressure_nonzero_guard')
    })
}

export function migrateG36PressureRequestHysteresis(saved: Project, reference: Block): Project {
  if (!needsG36PressureRequestHysteresisMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(isZone)) {
    const blocks = zone.blocks ?? []
    const positive = blocks.find(block => block.name === 'pressure_setpoint_positive')
    if (!positive?.block_type.endsWith('.Reals.Greater') || positive.parameters?.length ||
        !blocks.some(block => block.name === 'pressure_nonzero_guard')) continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    const threshold = template?.blocks?.find(block => block.name === 'pressure_setpoint_positive')
    if (!template || !threshold) throw new Error(`Pressure request hysteresis migration requires ${zone.name}`)
    Object.assign(positive, structuredClone(threshold), {notes: positive.notes || threshold.notes})
    zone.blocks = blocks.filter(block => block.name !== 'pressure_nonzero_guard')
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from !== 'pressure_nonzero_guard.y' && to !== 'pressure_setpoint_positive.u1' &&
        to !== 'pressure_setpoint_positive.u2'
    })
    zone.connections.push({from:'pressure_setpoint_sample.y',to:'pressure_setpoint_positive.u'})
    for (const name of ['pressure_below_half', 'pressure_below_seventy']) {
      const block = zone.blocks.find(item => item.name === name)
      if (block && !block.parameters?.some(parameter => parameter.name === 'h')) {
        const source = template.blocks?.find(item => item.name === name)
        const hysteresis = source?.parameters?.find(parameter => parameter.name === 'h')
        if (!hysteresis) throw new Error(`Pressure request hysteresis migration requires ${name}`)
        block.parameters = [...(block.parameters ?? []), structuredClone(hysteresis)]
      }
    }
  }
  return next
}

/** OBC uses the same damper hysteresis to qualify one, two, and three requests. */
export function needsG36PressureDamperLatchMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    (saved.graph.blocks ?? []).filter(isZone).some(zone => {
      const gate = zone.blocks?.find(block => block.name === 'pressure_damper_high_sampled')
      const latch = zone.blocks?.find(block => block.name === 'pressure_damper_hysteresis')
      return gate?.block_type.endsWith('.GreaterThreshold') &&
        gate.parameters?.find(parameter => parameter.name === 't')?.value === 0.95 &&
        gate.parameters?.find(parameter => parameter.name === 'h')?.value === 0.01 &&
        latch?.block_type.endsWith('.Hysteresis')
    })
}

export function migrateG36PressureDamperLatch(saved: Project): Project {
  if (!needsG36PressureDamperLatchMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of (next.graph.blocks ?? []).filter(isZone)) {
    const gate = zone.blocks?.find(block => block.name === 'pressure_damper_high_sampled')
    const latch = zone.blocks?.find(block => block.name === 'pressure_damper_hysteresis')
    if (!gate?.block_type.endsWith('.GreaterThreshold') ||
        gate.parameters?.find(parameter => parameter.name === 't')?.value !== 0.95 ||
        gate.parameters?.find(parameter => parameter.name === 'h')?.value !== 0.01 ||
        !latch?.block_type.endsWith('.Hysteresis')) continue
    const low = latch.parameters?.find(parameter => parameter.name === 'uLow')?.value
    const high = latch.parameters?.find(parameter => parameter.name === 'uHigh')?.value
    if (typeof low !== 'number' || typeof high !== 'number' || high <= low)
      throw new Error(`Pressure request latch migration requires valid ${zone.name} thresholds`)
    latch.block_type = 'Buildings.Controls.OBC.CDL.Reals.GreaterThreshold'
    latch.parameters = [{name:'t',type:'Real',value:high}, {name:'h',type:'Real',value:high-low}]
    latch.notes = `${latch.notes ?? ''} This one latch qualifies all three OBC pressure-request tiers.`.trim()
    zone.blocks = (zone.blocks ?? []).filter(block => block.name !== 'pressure_damper_high_sampled')
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return root(from) !== 'pressure_damper_high_sampled' &&
        root(to) !== 'pressure_damper_high_sampled' && to !== 'pressure_damper_delay.u'
    })
    zone.connections.push({from:'pressure_damper_hysteresis.y',to:'pressure_damper_delay.u'})
  }
  return next
}
