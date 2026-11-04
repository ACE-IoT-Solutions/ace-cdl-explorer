<template>
  <div class="signal-chart">
    <div class="signal-readout"><span>{{ hovered ? formatTime(hovered.index) : 'Drag across the chart to zoom into a time range' }}</span><strong v-if="hovered">{{ formatValue(hovered.trace ?? 0) }}</strong></div>
    <svg viewBox="0 0 900 260" preserveAspectRatio="none" role="img" :aria-label="`${label} from ${formatTime(start)} to ${formatTime(end)}. Drag to zoom.`" @pointerdown="beginDrag" @pointermove="movePointer" @pointerup="endDrag" @pointercancel="cancelDrag" @pointerleave="hoverIndex=null">
      <line v-for="n in 5" :key="n" x1="0" :y1="n*48" x2="900" :y2="n*48" stroke="rgba(255,255,255,.11)"/>
      <polyline :points="points" fill="none" stroke="var(--ace-primary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/>
      <rect v-if="dragStart!==null && dragEnd!==null" :x="Math.min(dragStart,dragEnd)*900" y="0" :width="Math.abs(dragEnd-dragStart)*900" height="260" fill="rgba(193,211,1,.18)" stroke="var(--ace-primary)"/>
      <line v-if="hovered && dragStart===null" :x1="xFor(hovered.index)" y1="0" :x2="xFor(hovered.index)" y2="260" stroke="var(--ace-secondary)" stroke-dasharray="4 4"/>
      <text x="8" y="20" fill="var(--ace-text-muted)" font-size="14">{{ formatValue(bounds.high) }}</text>
      <text x="8" y="248" fill="var(--ace-text-muted)" font-size="14">{{ formatValue(bounds.low) }}</text>
    </svg>
    <div class="chart-axis"><span>{{ formatTime(start) }}</span><span>{{ formatTime(Math.round((start+end)/2)) }}</span><span>{{ formatTime(end) }}</span></div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Row } from '../lib/types'

const props=defineProps<{ rows: Row[]; start: number; end: number; label: string }>()
const emit=defineEmits<{ range: [start:number,end:number] }>()
const hoverIndex=ref<number|null>(null)
const dragStart=ref<number|null>(null)
const dragEnd=ref<number|null>(null)
const visible=computed(()=>props.rows.filter(row=>row.index>=props.start&&row.index<=props.end&&typeof row.trace==='number'))
const sampled=computed(()=>{const rows=visible.value;const step=Math.max(1,Math.ceil(rows.length/850));const points=rows.filter((_,i)=>i%step===0);if(rows.length&&points.at(-1)!==rows.at(-1))points.push(rows.at(-1)!);return points})
const bounds=computed(()=>{const values=visible.value.map(row=>row.trace!);if(!values.length)return {low:0,high:1};let low=Math.min(...values),high=Math.max(...values);if(low===high){low-=0.5;high+=0.5}return {low,high}})
const hovered=computed(()=>hoverIndex.value===null?null:sampled.value.reduce<Row|null>((best,row)=>!best||Math.abs(row.index-hoverIndex.value!)<Math.abs(best.index-hoverIndex.value!)?row:best,null))
function xFor(index:number){return (index-props.start)/Math.max(1,props.end-props.start)*900}
const points=computed(()=>sampled.value.map(row=>`${xFor(row.index).toFixed(1)},${(230-(row.trace!-bounds.value.low)/(bounds.value.high-bounds.value.low)*205).toFixed(1)}`).join(' '))
function formatTime(index:number){return new Date(Date.UTC(2025,0,1)+index*900000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit',timeZone:'UTC'})}
function formatValue(value:number){return Number.isInteger(value)?value.toFixed(0):Math.abs(value)>=100?value.toFixed(1):value.toFixed(2)}
function fraction(event:PointerEvent){const box=(event.currentTarget as SVGElement).getBoundingClientRect();return Math.max(0,Math.min(1,(event.clientX-box.left)/box.width))}
function indexAt(fraction:number){return Math.round(props.start+fraction*(props.end-props.start))}
function beginDrag(event:PointerEvent){if(event.button!==0)return;(event.currentTarget as SVGElement).setPointerCapture(event.pointerId);dragStart.value=fraction(event);dragEnd.value=dragStart.value;hoverIndex.value=indexAt(dragStart.value)}
function movePointer(event:PointerEvent){const at=fraction(event);hoverIndex.value=indexAt(at);if(dragStart.value!==null)dragEnd.value=at}
function endDrag(event:PointerEvent){if(dragStart.value===null)return;const end=fraction(event);const first=Math.min(dragStart.value,end),last=Math.max(dragStart.value,end);if(last-first>0.01)emit('range',indexAt(first),indexAt(last));dragStart.value=null;dragEnd.value=null}
function cancelDrag(){dragStart.value=null;dragEnd.value=null}
</script>
