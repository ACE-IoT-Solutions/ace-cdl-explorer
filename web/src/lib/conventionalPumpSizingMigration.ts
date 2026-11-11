import type { Project } from './types'

const oldGain = 1 / 180
const newGain = 1 / 90
const oldNote = 'Sizes hot-water pump speed from requested load using the 180 kW design flow.'
const newNote = 'Sizes hot-water pump speed from requested load using a 90 kW branch-service design load; physical bypass flow does not deliver reheat.'

/** Update only the bundled conventional gain; preserve every custom value. */
export function needsConventionalPumpSizingMigration(project: Project): boolean {
  if (project.graph.name !== 'FourPipeBuildingControls') return false
  const plant = project.graph.blocks?.find(block => block.name === 'plant')
  const gain = plant?.blocks?.find(block => block.name === 'hot_water_pump_gain')
  return gain?.parameters?.find(parameter => parameter.name === 'k')?.value === oldGain
}

export function migrateConventionalPumpSizing(project: Project): Project {
  if (!needsConventionalPumpSizingMigration(project)) return project
  const next = structuredClone(project)
  const gain = next.graph.blocks?.find(block => block.name === 'plant')?.blocks?.find(block => block.name === 'hot_water_pump_gain')
  const parameter = gain?.parameters?.find(item => item.name === 'k')
  if (parameter) parameter.value = newGain
  if (gain?.notes === oldNote) gain.notes = newNote
  return next
}
