import type { Project } from './types'

/** Match the default mode code to any positive heat-pump command. */
export function needsHeatPumpModeThresholdMigration(project: Project): boolean {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(project.graph.name)) return false
  const plant = project.graph.blocks?.find(block => block.name === 'plant')
  const threshold = plant?.blocks?.find(block => block.name === 'heat_pump_mode_threshold')
  return threshold?.parameters?.find(parameter => parameter.name === 'k')?.value === 0.5
}

export function migrateHeatPumpModeThreshold(project: Project): Project {
  if (!needsHeatPumpModeThresholdMigration(project)) return project
  const next = structuredClone(project)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const threshold = plant?.blocks?.find(block => block.name === 'heat_pump_mode_threshold')
  const parameter = threshold?.parameters?.find(item => item.name === 'k')
  if (parameter) parameter.value = 0
  return next
}
