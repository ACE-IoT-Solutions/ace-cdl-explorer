import type { Summary } from './types'

export const summaryMetrics: { key: keyof Summary; label: string; unit: string; detail: string }[] = [
  { key: 'electricity_kwh', label: 'Electricity', unit: 'kWh', detail: 'Heat pump, chiller, fans, and water pumps' },
  { key: 'gas_kwh', label: 'Boiler fuel', unit: 'kWh', detail: 'Fuel input' },
  { key: 'occupied_unmet_zone_hours', label: 'Comfort exceedance · present zones', unit: 'zone-hours', detail: 'Raw presence during scheduled occupancy · 20–25°C' },
  { key: 'unmet_zone_hours', label: 'Comfort exceedance · all zones', unit: 'zone-hours', detail: 'Every zone, all year · 20–25°C scheduled / 17–29°C otherwise' },
]

export function formatSummaryValue(value: number, metric: keyof Summary): string {
  const comfort = metric === 'unmet_zone_hours' || metric === 'occupied_unmet_zone_hours'
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: comfort ? 2 : 0 }).format(value)
}
