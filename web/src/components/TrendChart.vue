<template>
  <div class="trend-chart"><div class="chart-legend"><span><i class="legend-lime"/>{{ labelA }}</span><span><i class="legend-cyan"/>{{ labelB }}</span><b>{{ unit }}</b></div><svg viewBox="0 0 920 260" preserveAspectRatio="none" role="img" :aria-label="`${labelA} and ${labelB} in ${unit} from ${formatTime(start)} to ${formatTime(end)}`"><g v-for="n in 5" :key="n"><line x1="64" :y1="20+(n-1)*50" x2="900" :y2="20+(n-1)*50" stroke="rgba(255,255,255,.10)"/><text x="56" :y="24+(n-1)*50" text-anchor="end" fill="var(--ace-text-muted)" font-size="12">{{ formatValue(bounds.high-(n-1)*(bounds.high-bounds.low)/4) }}</text></g><line x1="64" y1="20" x2="64" y2="220" stroke="var(--ace-border-strong)"/><line x1="64" y1="220" x2="900" y2="220" stroke="var(--ace-border-strong)"/><polyline :points="pointsA" fill="none" stroke="var(--ace-primary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/><polyline :points="pointsB" fill="none" stroke="var(--ace-secondary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg><div class="chart-axis"><span>{{ formatTime(start) }}</span><span>{{ formatTime(Math.round((start+end)/2)) }}</span><span>{{ formatTime(end) }}</span></div></div>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import type { Row } from '../lib/types'
type NumericField = 'avg_zone_c'|'outdoor_c'|'electric_kw'|'gas_kw'|'heat_kw'|'cool_kw'|'requested_airflow'|'delivered_airflow'|'ahu_1_supply_actual_c'|'ahu_1_supply_setpoint_c'|'ahu_2_supply_actual_c'|'ahu_2_supply_setpoint_c'
const props=defineProps<{ rows: Row[]; start: number; end: number; fieldA: NumericField; fieldB: NumericField; labelA: string; labelB: string; unit: string; zeroBased?: boolean }>()
const sampled=computed(()=>{const rows=props.rows.filter(row=>row.index>=props.start&&row.index<=props.end);const step=Math.max(1,Math.ceil(rows.length/450));const points=rows.filter((_,i)=>i%step===0);if(rows.length&&points.at(-1)!==rows.at(-1))points.push(rows.at(-1)!);return points})
const bounds=computed(()=>{const all=sampled.value.flatMap(r=>[r[props.fieldA],r[props.fieldB]]);if(!all.length)return {low:0,high:1};let low=Math.min(...all),high=Math.max(...all);if(props.zeroBased)low=Math.min(0,low);if(low===high){low-=.5;high+=.5}return {low,high}})
function points(field: typeof props.fieldA|typeof props.fieldB){const a=sampled.value;const span=bounds.value.high-bounds.value.low||1;return a.map(r=>`${(64+(r.index-props.start)/Math.max(1,props.end-props.start)*836).toFixed(1)},${(220-(r[field]-bounds.value.low)/span*200).toFixed(1)}`).join(' ')}
const pointsA=computed(()=>points(props.fieldA))
const pointsB=computed(()=>points(props.fieldB))
function formatTime(index:number){return new Date(Date.UTC(2025,0,1)+index*900000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit',timeZone:'UTC'})}
function formatValue(value:number){return Math.abs(value)>=100?value.toFixed(0):Math.abs(value)>=10?value.toFixed(1):value.toFixed(2)}
</script>
