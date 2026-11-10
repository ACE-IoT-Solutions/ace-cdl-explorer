import type { Summary } from './types'

export function formatSummaryValue(value: number, metric: keyof Summary): string {
  const comfort = metric === 'unmet_zone_hours' || metric === 'occupied_unmet_zone_hours'
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: comfort ? 2 : 0 }).format(value)
}
