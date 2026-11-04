<template>
  <div class="trend-chart"><div class="chart-legend"><span><i class="legend-lime"/>{{ labelA }}</span><span><i class="legend-cyan"/>{{ labelB }}</span></div><svg viewBox="0 0 900 230" preserveAspectRatio="none" role="img" :aria-label="`${labelA} and ${labelB} from ${formatTime(start)} to ${formatTime(end)}`"><line v-for="n in 5" :key="n" x1="0" :y1="n*42" x2="900" :y2="n*42" stroke="rgba(255,255,255,.10)"/><polyline :points="pointsA" fill="none" stroke="var(--ace-primary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/><polyline :points="pointsB" fill="none" stroke="var(--ace-secondary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg><div class="chart-axis"><span>{{ formatTime(start) }}</span><span>{{ formatTime(Math.round((start+end)/2)) }}</span><span>{{ formatTime(end) }}</span></div></div>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import type { Row } from '../lib/types'
const props=defineProps<{ rows: Row[]; start: number; end: number; fieldA: 'avg_zone_c'|'electric_kw'|'heat_kw'|'cool_kw'; fieldB: 'outdoor_c'|'gas_kw'|'cool_kw'|'heat_kw'; labelA: string; labelB: string }>()
const sampled=computed(()=>{const rows=props.rows.filter(row=>row.index>=props.start&&row.index<=props.end);const step=Math.max(1,Math.ceil(rows.length/450));const points=rows.filter((_,i)=>i%step===0);if(rows.length&&points.at(-1)!==rows.at(-1))points.push(rows.at(-1)!);return points})
const bounds=computed(()=>{const all=sampled.value.flatMap(r=>[r[props.fieldA],r[props.fieldB]]);return {low:Math.min(...all,0),high:Math.max(...all,1)}})
function points(field: typeof props.fieldA|typeof props.fieldB){const a=sampled.value;const span=bounds.value.high-bounds.value.low||1;return a.map(r=>`${((r.index-props.start)/Math.max(1,props.end-props.start)*900).toFixed(1)},${(200-(r[field]-bounds.value.low)/span*180).toFixed(1)}`).join(' ')}
const pointsA=computed(()=>points(props.fieldA))
const pointsB=computed(()=>points(props.fieldB))
function formatTime(index:number){return new Date(Date.UTC(2025,0,1)+index*900000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit',timeZone:'UTC'})}
</script>
