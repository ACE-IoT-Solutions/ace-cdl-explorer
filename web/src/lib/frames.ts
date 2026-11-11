import { tableFromIPC, type Table, type Vector } from 'apache-arrow'
import type { Row } from './types'

export interface RowSource {
  readonly length: number
  at(index: number): Row | undefined
  sample(start: number, end: number, limit: number): Row[]
}

export class ArrayRows implements RowSource {
  constructor(private readonly rows: Row[]) {}
  get length() { return this.rows.length }
  at(index: number) { return this.rows.at(index) }
  sample(start: number, end: number, limit: number) {
    const visible = this.rows.filter(row => row.index >= start && row.index <= end)
    const stride = Math.max(1, Math.ceil(visible.length / limit))
    const sampled = visible.filter((_, index) => index % stride === 0)
    if (visible.length && sampled.at(-1) !== visible.at(-1)) sampled.push(visible.at(-1)!)
    return sampled
  }
}

export const measures = [
  'schedule_occupied',
  'preconditioning_mode',
  'outdoor_c', 'ahu_1_return_air_relative_humidity', 'ahu_2_return_air_relative_humidity', 'avg_zone_c', 'cool_kw', 'heat_kw', 'cool_request_kw',
  'heat_request_kw', 'electric_kw', 'gas_kw', 'requested_airflow',
  'delivered_airflow', 'ahu_1_supply_actual_c', 'ahu_2_supply_actual_c',
  'ahu_1_supply_setpoint_c', 'ahu_2_supply_setpoint_c',
  'ahu_1_duct_pressure', 'ahu_2_duct_pressure',
  'ahu_1_outdoor_air_fraction', 'ahu_2_outdoor_air_fraction',
  'ahu_1_intake_opening', 'ahu_2_intake_opening',
  'ahu_1_actual_fan_speed', 'ahu_2_actual_fan_speed',
  'ahu_1_measured_fan_speed', 'ahu_2_measured_fan_speed',
  'ahu_1_actual_outdoor_damper_position', 'ahu_2_actual_outdoor_damper_position',
  'ahu_1_actual_return_damper_position', 'ahu_2_actual_return_damper_position',
  'ahu_1_measured_outdoor_damper_position', 'ahu_2_measured_outdoor_damper_position',
  'ahu_1_measured_return_damper_position', 'ahu_2_measured_return_damper_position',
  'ahu_1_heating_coil_valve_position', 'ahu_2_heating_coil_valve_position',
  'ahu_1_cooling_coil_valve_position', 'ahu_2_cooling_coil_valve_position',
  'ahu_1_heating_coil_request_kw', 'ahu_2_heating_coil_request_kw',
  'ahu_1_cooling_coil_request_kw', 'ahu_2_cooling_coil_request_kw',
  'ahu_1_heating_coil_delivered_kw', 'ahu_2_heating_coil_delivered_kw',
  'ahu_1_cooling_coil_delivered_kw', 'ahu_2_cooling_coil_delivered_kw',
  'ahu_1_outdoor_airflow', 'ahu_2_outdoor_airflow',
  'ahu_1_mixed_air_c', 'ahu_2_mixed_air_c',
  'ahu_1_measured_mixed_air_c', 'ahu_2_measured_mixed_air_c',
  'heat_pump_heating_used_kw', 'heat_pump_cooling_used_kw', 'boiler_used_kw',
  'heat_pump_electric_kw', 'heat_pump_ambient_exchange_kw',
  'heat_pump_available_capacity_kw', 'heat_pump_heating_available_capacity_kw', 'heat_pump_cooling_available_capacity_kw', 'heat_pump_part_load_fraction', 'heat_pump_cop_multiplier', 'heat_pump_hot_water_cop_multiplier', 'heat_pump_chilled_water_cop_multiplier', 'heat_pump_heating_cop', 'heat_pump_cooling_cop',
  'heat_pump_available', 'boiler_available', 'chiller_available',
  'heat_pump_running', 'boiler_running', 'chiller_running', 'hot_water_plant_proven',
  'plant_heat_pump_start_fault',
  'plant_boiler_start_fault', 'plant_chiller_start_fault',
  'chiller_used_kw',
  'hot_water_supply_temp_c', 'hot_water_supply_setpoint_c', 'hot_water_return_temp_c',
  'chilled_water_supply_temp_c', 'chilled_water_supply_setpoint_c', 'chilled_water_return_temp_c',
  'hot_water_flow_kg_s', 'chilled_water_flow_kg_s',
  'hot_water_actual_flow_kg_s', 'chilled_water_actual_flow_kg_s',
  'hot_water_bypass_flow_kg_s', 'chilled_water_bypass_flow_kg_s',
  'ahu_1_hot_water_flow_kg_s', 'ahu_2_hot_water_flow_kg_s',
  'ahu_1_chilled_water_flow_kg_s', 'ahu_2_chilled_water_flow_kg_s',
  'hot_water_reheat_flow_kg_s',
  'hot_water_differential_pressure_pa', 'chilled_water_differential_pressure_pa',
  'hot_water_pump_speed', 'chilled_water_pump_speed',
  'hot_water_pump_running', 'chilled_water_pump_running',
  'hot_water_enable', 'chilled_water_enable', 'water_pump_kw',
] as const

type Measure = typeof measures[number]

export class ArrowRows implements RowSource {
  private batches: Array<{ start: number; table: Table; columns: Map<string, Vector> }> = []
  length = 0
  constructor(private readonly traceIds: string[]) {}

  append(buffer: ArrayBuffer) {
    if (!buffer.byteLength) return
    const table = tableFromIPC(new Uint8Array(buffer))
    if (!table.numRows) return
    const first = Number(table.getChild('index')?.get(0))
    if (first !== this.length) throw new Error(`Unexpected Arrow row index ${first}; expected ${this.length}`)
    this.batches.push({ start: this.length, table, columns: new Map() })
    this.length += table.numRows
  }

  at(index: number): Row | undefined {
    if (index < 0) index += this.length
    if (index < 0 || index >= this.length) return undefined
    let low = 0, high = this.batches.length - 1
    while (low < high) {
      const mid = (low + high + 1) >> 1
      if (this.batches[mid].start <= index) low = mid
      else high = mid - 1
    }
    const { start, table, columns } = this.batches[low]
    const offset = index - start
    const value = (name: string) => {
      let column = columns.get(name)
      if (!column) {
        column = table.getChild(name) ?? undefined
        if (!column) throw new Error(`Missing Arrow column ${name}`)
        columns.set(name, column)
      }
      return Number(column.get(offset))
    }
    const row = { index: value('index'), zone_c: Array.from({ length: 10 }, (_, zone) => value(`zone_${String(zone + 1).padStart(2, '0')}_c`)), zone_co2_ppm:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_co2_ppm`)), zone_relative_humidity:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_relative_humidity`)), zone_occupied:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_occupied`)), zone_presence_raw:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_presence_raw`)), zone_occupant_count_actual:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_occupant_count_actual`)), zone_occupant_count_sensor:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_occupant_count_sensor`)), zone_occupant_sensible_heat_kw:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_occupant_sensible_heat_kw`)), zone_airflow:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_airflow`)), zone_discharge_c:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_discharge_c`)), zone_actual_damper_position:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_actual_damper_position`)), zone_actual_reheat_valve_position:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_actual_reheat_valve_position`)), zone_measured_reheat_valve_position:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_measured_reheat_valve_position`)) } as Row
    for (const name of measures) row[name as Measure] = value(name)
    if (this.traceIds.length) row.traces = Object.fromEntries(this.traceIds.map(id => [id, value(`trace:${id}`)]))
    return row
  }

  sample(start: number, end: number, limit: number): Row[] {
    const first = Math.max(0, Math.ceil(start))
    const last = Math.min(this.length - 1, Math.floor(end))
    if (last < first) return []
    const stride = Math.max(1, Math.ceil((last - first + 1) / limit))
    const rows: Row[] = []
    for (let index = first; index <= last; index += stride) rows.push(this.at(index)!)
    if (rows.at(-1)?.index !== last) rows.push(this.at(last)!)
    return rows
  }
}
