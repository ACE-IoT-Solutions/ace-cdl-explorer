<template>
  <div class="chart-card signal-plot">
    <div class="card-heading"><div><span class="eyebrow">SIGNAL GROUP · {{ unit }}</span><h2>{{ unit }} over time</h2></div><span class="muted small">{{ signals.length }} {{ signals.length===1 ? 'signal' : 'signals' }}</span></div>
    <div class="signal-legend"><span v-for="series in plotted" :key="series.id"><i :style="{background:series.color}"/>{{ series.label }}</span></div>
    <div class="signal-chart">
      <div class="signal-readout"><span>{{ hovered ? formatTime(hovered.index) : panMode ? 'Drag to pan the time range' : 'Drag to zoom · Shift+drag to pan' }}</span><div v-if="hovered"><span v-for="series in plotted" :key="series.id" :style="{color:series.color}">{{ series.label }}: {{ readValue(hovered,series.id) }}</span></div></div>
      <svg viewBox="0 0 920 280" preserveAspectRatio="none" role="img" :class="{'pan-mode':panMode,dragging:panStart!==null}" :aria-label="`${unit} signals from ${formatTime(start)} to ${formatTime(end)}. Drag to zoom; use Pan charts or Shift+drag to pan.`" @pointerdown="beginDrag" @pointermove="movePointer" @pointerup="endDrag" @pointercancel="cancelDrag" @pointerleave="hoverIndex=null">
        <g v-for="n in 5" :key="n"><line x1="64" :y1="20+(n-1)*55" x2="900" :y2="20+(n-1)*55" stroke="rgba(255,255,255,.11)"/><text x="56" :y="24+(n-1)*55" text-anchor="end" fill="var(--ace-text-muted)" font-size="12">{{ formatValue(bounds.high-(n-1)*(bounds.high-bounds.low)/4) }}</text></g>
        <line x1="64" y1="20" x2="64" y2="240" stroke="var(--ace-border-strong)"/><line x1="64" y1="240" x2="900" y2="240" stroke="var(--ace-border-strong)"/>
        <polyline v-for="series in plotted" :key="series.id" :points="series.points" fill="none" :stroke="series.color" stroke-width="2.5" vector-effect="non-scaling-stroke"/>
        <rect v-if="dragStart!==null && dragEnd!==null" :x="xAt(Math.min(dragStart,dragEnd))" y="20" :width="Math.abs(dragEnd-dragStart)*836" height="220" fill="rgba(193,211,1,.18)" stroke="var(--ace-primary)"/>
        <line v-if="hovered && dragStart===null" :x1="xFor(hovered.index)" y1="20" :x2="xFor(hovered.index)" y2="240" stroke="var(--ace-text-muted)" stroke-dasharray="4 4"/>
        <text x="64" y="268" fill="var(--ace-text-muted)" font-size="12">{{ formatTime(start) }}</text><text x="900" y="268" text-anchor="end" fill="var(--ace-text-muted)" font-size="12">{{ formatTime(end) }}</text>
      </svg>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { signalValue, type GraphSignal } from '../lib/signals'
import type { Row } from '../lib/types'
import type { RowSource } from '../lib/frames'

const props=defineProps<{ rows: RowSource; version: number; start: number; end: number; unit: string; signals: GraphSignal[]; panMode: boolean }>()
const emit=defineEmits<{ range: [start:number,end:number]; pan: [start:number] }>()
const colors=['var(--ace-primary)','var(--ace-secondary)','#ff956c','#b89cff','#ffe082','#64d4aa','#ff75ad']
const hoverIndex=ref<number|null>(null),dragStart=ref<number|null>(null),dragEnd=ref<number|null>(null)
const panStart=ref<{x:number;start:number;span:number;width:number}|null>(null)
const sampled=computed(()=>{props.version;return props.rows.sample(props.start,props.end,700)})
const bounds=computed(()=>{if(props.unit==='Boolean (0/1)'||props.unit==='fraction (0–1)')return {low:0,high:1};const values=sampled.value.flatMap(row=>props.signals.flatMap(signal=>{const value=signalValue(row,signal.id);return value===undefined?[]:[value]}));if(!values.length)return {low:0,high:1};let low=Math.min(...values),high=Math.max(...values);if(low===high){low-=0.5;high+=0.5}const padding=(high-low)*.05;return {low:low-padding,high:high+padding}})
const plotted=computed(()=>props.signals.map((signal,index)=>({id:signal.id,label:signal.label,color:colors[index%colors.length],points:sampled.value.flatMap(row=>{const value=signalValue(row,signal.id);return value===undefined?[]:[`${xFor(row.index).toFixed(1)},${yFor(value).toFixed(1)}`]}).join(' ')})))
const hovered=computed(()=>hoverIndex.value===null?null:sampled.value.reduce<Row|null>((best,row)=>!best||Math.abs(row.index-hoverIndex.value!)<Math.abs(best.index-hoverIndex.value!)?row:best,null))
function xAt(fraction:number){return 64+fraction*836}
function xFor(index:number){return xAt((index-props.start)/Math.max(1,props.end-props.start))}
function yFor(value:number){return 240-(value-bounds.value.low)/(bounds.value.high-bounds.value.low)*220}
function formatTime(index:number){return new Date(Date.UTC(2025,0,1)+index*900000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit',timeZone:'UTC'})}
function formatValue(value:number){return Math.abs(value)>=100?value.toFixed(0):Math.abs(value)>=10?value.toFixed(1):value.toFixed(2)}
function readValue(row:Row,id:string){const value=signalValue(row,id);return value===undefined?'—':formatValue(value)}
function fraction(event:PointerEvent){const box=(event.currentTarget as SVGElement).getBoundingClientRect();return Math.max(0,Math.min(1,((event.clientX-box.left)/box.width*920-64)/836))}
function indexAt(value:number){return Math.round(props.start+value*(props.end-props.start))}
function beginDrag(event:PointerEvent){if(event.button!==0)return;const svg=event.currentTarget as SVGElement;svg.setPointerCapture(event.pointerId);if(props.panMode||event.shiftKey){const box=svg.getBoundingClientRect();panStart.value={x:event.clientX,start:props.start,span:props.end-props.start,width:box.width*836/920};hoverIndex.value=null;return}dragStart.value=fraction(event);dragEnd.value=dragStart.value;hoverIndex.value=indexAt(dragStart.value)}
function movePointer(event:PointerEvent){if(panStart.value){const pan=panStart.value;emit('pan',pan.start-(event.clientX-pan.x)/Math.max(1,pan.width)*pan.span);return}const at=fraction(event);hoverIndex.value=indexAt(at);if(dragStart.value!==null)dragEnd.value=at}
function endDrag(event:PointerEvent){if(panStart.value){movePointer(event);panStart.value=null;return}if(dragStart.value===null)return;const end=fraction(event),first=Math.min(dragStart.value,end),last=Math.max(dragStart.value,end);if(last-first>0.01)emit('range',indexAt(first),indexAt(last));dragStart.value=null;dragEnd.value=null}
function cancelDrag(){dragStart.value=null;dragEnd.value=null;panStart.value=null}
</script>

<style scoped>
svg.pan-mode{cursor:grab}
svg.dragging{cursor:grabbing}
</style>
