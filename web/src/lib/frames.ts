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

const measures = [
  'schedule_occupied',
  'preconditioning_mode',
  'outdoor_c', 'avg_zone_c', 'cool_kw', 'heat_kw', 'cool_request_kw',
  'heat_request_kw', 'electric_kw', 'gas_kw', 'requested_airflow',
  'delivered_airflow', 'ahu_1_supply_actual_c', 'ahu_2_supply_actual_c',
  'ahu_1_supply_setpoint_c', 'ahu_2_supply_setpoint_c',
  'ahu_1_duct_pressure', 'ahu_2_duct_pressure',
  'ahu_1_outdoor_air_fraction', 'ahu_2_outdoor_air_fraction',
  'ahu_1_heating_coil_valve_position', 'ahu_2_heating_coil_valve_position',
  'ahu_1_cooling_coil_valve_position', 'ahu_2_cooling_coil_valve_position',
  'ahu_1_heating_coil_request_kw', 'ahu_2_heating_coil_request_kw',
  'ahu_1_cooling_coil_request_kw', 'ahu_2_cooling_coil_request_kw',
  'ahu_1_heating_coil_delivered_kw', 'ahu_2_heating_coil_delivered_kw',
  'ahu_1_cooling_coil_delivered_kw', 'ahu_2_cooling_coil_delivered_kw',
  'ahu_1_outdoor_airflow', 'ahu_2_outdoor_airflow',
  'ahu_1_mixed_air_c', 'ahu_2_mixed_air_c',
  'heat_pump_heating_used_kw', 'heat_pump_cooling_used_kw', 'boiler_used_kw',
  'chiller_used_kw',
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
    const row = { index: value('index'), zone_c: Array.from({ length: 10 }, (_, zone) => value(`zone_${String(zone + 1).padStart(2, '0')}_c`)), zone_occupied:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_occupied`)), zone_presence_raw:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_presence_raw`)), zone_airflow:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_airflow`)), zone_discharge_c:Array.from({length:10},(_,zone)=>value(`zone_${String(zone+1).padStart(2,'0')}_discharge_c`)) } as Row
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
