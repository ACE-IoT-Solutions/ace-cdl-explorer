import { edgeEndpoints, type Block, type Project } from './types'

const legacyWeather = new Set(['supply_reset', 'warmup_supply', 'mild_or_cool', 'mild_supply',
  'cooling_supply', 'is_cold', 'is_mild', 'cold_threshold', 'warm_threshold'])
const newNames = new Set(['sat_maximum', 'sat_outdoor_low', 'sat_outdoor_high',
  'sat_request_sample', 'sat_weather_line'])

export function needsG36ObcSupplyAirResetMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    [1, 2].some(index => {
      const blocks = project.graph.blocks?.find(block => block.name === `ahu_${index}`)?.blocks
      return !blocks?.some(block => block.name === 'sat_weather_line') ||
        blocks.some(block => legacyWeather.has(block.name))
    })
}

/** Replace the old stepped weather target with OBC's sampled maximum and Line. */
export function migrateG36ObcSupplyAirReset(saved: Project, reference: Block): Project {
  if (!needsG36ObcSupplyAirResetMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const ahu = next.graph.blocks?.find(block => block.name === `ahu_${index}`)
    const template = reference.blocks?.find(block => block.name === `ahu_${index}`)
    if (!ahu || !template) throw new Error(`OBC supply-air reset migration requires ahu_${index}`)
    if (ahu.blocks?.some(block => block.name === 'sat_weather_line') &&
        !ahu.blocks.some(block => legacyWeather.has(block.name))) continue
    const child = (name: string) => ahu.blocks?.find(block => block.name === name)
    const parameter = (name: string, key: string) => child(name)?.parameters?.find(item => item.name === key)
    const formerMaximum = parameter('warmup_supply', 'k')?.value
    const minimum = parameter('sat_minimum', 'k')
    if (minimum?.value === 14) minimum.value = 12
    const initial = parameter('sat_previous', 'y_start')
    if (initial?.value === 16) initial.value = 18
    const delay = parameter('sat_fan_delay', 'delayTime')
    if (delay?.value === 600) delay.value = 720
    ahu.blocks = (ahu.blocks ?? []).filter(block => !legacyWeather.has(block.name))
    for (const block of template.blocks ?? []) {
      if (newNames.has(block.name) && !ahu.blocks.some(existing => existing.name === block.name))
        ahu.blocks.push(structuredClone(block))
    }
    if (typeof formerMaximum === 'number' && formerMaximum !== 18) {
      const maximum = parameter('sat_maximum', 'k')
      if (maximum) maximum.value = formerMaximum
    }
    const obsolete = new Set([
      'sat_request_real.y→sat_excess_requests.u1',
      'fan_requested.y→sat_fan_delay.u',
      'sat_previous.y→sat_output.u1',
      'sat_output.y→sat_mode_weather_select.u1',
    ])
    ahu.connections = (ahu.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !legacyWeather.has(from.split('.')[0]) && !legacyWeather.has(to.split('.')[0]) &&
        !obsolete.has(`${from}→${to}`)
    })
    const present = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      const wanted = newNames.has(from.split('.')[0]) || newNames.has(to.split('.')[0]) ||
        ['sat_excess_requests.u1', 'sat_fan_delay.u', 'sat_output.u1', 'sat_mode_weather_select.u1']
          .includes(to)
      if (!wanted || present.has(`${from}→${to}`)) continue
      ahu.connections.push(structuredClone(edge))
      present.add(`${from}→${to}`)
    }
    ahu.notes = `${ahu.notes ?? ''} OBC's cooling maximum now samples VAV requests independently of outdoor weather; Line resets it over 16–21 °C.`.trim()
  }
  return next
}
