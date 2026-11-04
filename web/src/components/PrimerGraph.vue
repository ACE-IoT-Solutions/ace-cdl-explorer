<template>
  <div class="primer-graph-panel">
    <div class="primer-graph-toolbar"><span>{{ variant==='demand' ? 'VAV COMPOSITE / COOLING DEMAND' : 'VAV COMPOSITE / COOLING MODE' }}</span><span>LIVE · READ ONLY <span class="primer-pan-hint">· Drag to pan</span></span></div>
    <div ref="viewport" class="primer-graph-viewport" @scroll="updateOverflow" @pointerdown="startPan" @pointermove="movePan" @pointerup="stopPan" @pointercancel="stopPan">
      <div class="graph-canvas primer-graph-canvas" :style="{width:'1060px',height:'420px'}">
        <svg class="edge-layer" width="1060" height="420" aria-hidden="true">
          <g v-for="(path,index) in paths" :key="index"><path :d="path" fill="none" stroke="var(--ace-secondary)" stroke-width="2" opacity=".8"/><circle :cx="endpoints[index][0]" :cy="endpoints[index][1]" r="3" fill="var(--ace-secondary)"/></g>
        </svg>
        <div v-for="boundary in boundaries" :key="boundary.name" class="primer-boundary" :style="{left:boundary.x+'px',top:boundary.y+'px'}"><span>{{ boundary.name }}</span><b>{{ boundary.value }}</b></div>
        <article v-for="node in nodes" :key="node.name" class="graph-node primer-graph-node" :style="{left:node.x+'px',top:node.y+'px'}" @mouseenter="activeTooltip=node.name" @mouseleave="activeTooltip=null">
          <div class="node-head"><span>{{ node.name }}</span><span class="node-kind">{{ node.kind }}</span><button class="node-info" type="button" :aria-label="`About ${node.name}`" @click.stop="activeTooltip=activeTooltip===node.name?null:node.name">i</button></div>
          <div class="node-ports"><div class="input-ports"><div v-for="port in node.inputs" :key="port.name" class="port-row"><i class="port-dot input-dot"/><span>{{ port.name }} <b>{{ port.value }}</b></span></div></div><div class="output-ports"><div v-for="port in node.outputs" :key="port.name" class="port-row"><span>{{ port.name }} <b>{{ port.value }}</b></span><i class="port-dot output-dot"/></div></div></div>
          <div v-if="node.setting" class="primer-node-setting">{{ node.setting }}</div>
          <div v-if="activeTooltip===node.name" class="node-tooltip" :class="{'tooltip-left':node.x>650}" role="tooltip"><strong>{{ node.name }}</strong><span class="tooltip-kind">{{ node.kind }}</span><p>{{ node.description }}</p><div class="tooltip-group"><b>LIVE INPUTS</b><div v-for="port in node.inputs" :key="port.name" class="tooltip-port">{{ port.name }} · {{ port.value }}</div></div><div class="tooltip-group"><b>LIVE OUTPUTS</b><div v-for="port in node.outputs" :key="port.name" class="tooltip-port">{{ port.name }} · {{ port.value }}</div></div></div>
        </article>
      </div>
    </div>
    <button v-if="canPanLeft" class="primer-graph-pan primer-graph-pan-left" type="button" aria-label="More primer graph to the left" @click="panBy(-1)">←</button>
    <button v-if="canPanRight" class="primer-graph-pan primer-graph-pan-right" type="button" aria-label="More primer graph to the right" @click="panBy(1)">→</button>
    <div class="primer-graph-caption"><span>{{ variant==='demand' ? 'The same cool_error → cool_demand_gain → cool_demand_limit path is inside each VAV in the editor.' : 'The Boolean cooling_mode output turns true only when cooling error exceeds the 0.5 K deadband.' }}</span><strong>{{ variant==='demand' ? `cooling_demand = ${demand.toFixed(2)}` : `cooling_mode = ${coolingMode}` }}</strong></div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { blockCatalog } from '../lib/blockCatalog'

interface Port { name:string; value:string }
interface Node { name:string; kind:string; x:number; y:number; inputs:Port[]; outputs:Port[]; setting?:string; description:string }
const props=defineProps<{variant:'demand'|'mode'; measured:number; setpoint:number; gain:number}>()
const error=computed(()=>props.measured-props.setpoint)
const scaled=computed(()=>error.value*props.gain)
const demand=computed(()=>Math.max(0,Math.min(1,scaled.value)))
const coolingMode=computed(()=>error.value>0.5)
const description=(kind:string)=>blockCatalog.find(item=>item.type===kind)?.description??''
const temp=(value:number)=>`${value.toFixed(1)} °C`
const difference=(value:number)=>`${value.toFixed(1)} K`
const fraction=(value:number)=>value.toFixed(2)
const nodes=computed<Node[]>(()=>props.variant==='demand'?[
  {name:'cool_setpoint',kind:'Constant',x:30,y:30,inputs:[],outputs:[{name:'y',value:temp(props.setpoint)}],setting:`k = ${temp(props.setpoint)}`,description:description('Constant')},
  {name:'cool_error',kind:'Subtract',x:370,y:30,inputs:[{name:'u1',value:temp(props.measured)},{name:'u2',value:temp(props.setpoint)}],outputs:[{name:'y',value:difference(error.value)}],description:description('Subtract')},
  {name:'cool_demand_gain',kind:'MultiplyByParameter',x:710,y:30,inputs:[{name:'u',value:difference(error.value)}],outputs:[{name:'y',value:fraction(scaled.value)}],setting:`k = ${props.gain.toFixed(2)} K⁻¹`,description:description('MultiplyByParameter')},
  {name:'cool_demand_limit',kind:'Limiter',x:710,y:230,inputs:[{name:'u',value:fraction(scaled.value)}],outputs:[{name:'y',value:fraction(demand.value)}],setting:'uMin = 0 · uMax = 1',description:description('Limiter')},
]:[
  {name:'cool_error',kind:'Subtract',x:160,y:70,inputs:[{name:'u1',value:temp(props.measured)},{name:'u2',value:temp(props.setpoint)}],outputs:[{name:'y',value:difference(error.value)}],description:description('Subtract')},
  {name:'cool_deadband',kind:'Constant',x:450,y:230,inputs:[],outputs:[{name:'y',value:'0.5 K'}],setting:'k = 0.5 K',description:description('Constant')},
  {name:'cooling_mode',kind:'Greater',x:740,y:70,inputs:[{name:'u1',value:difference(error.value)},{name:'u2',value:'0.5 K'}],outputs:[{name:'y',value:String(coolingMode.value)}],description:description('Greater')},
])
const paths=computed(()=>props.variant==='demand'?[
  'M330 27 L330 99 L375 99',
  'M306 99 C340 99 340 123 375 123',
  'M646 99 L715 99',
  'M986 99 L1025 99 L1025 205 L690 205 L690 298 L715 298',
  'M986 298 L1040 298',
]:[
  'M125 139 L165 139',
  'M125 163 L165 163',
  'M436 139 L745 139',
  'M726 299 L730 299 L730 163 L745 163',
  'M1016 139 L1040 139',
])
const endpoints=computed<[number,number][]>(()=>props.variant==='demand'?[[375,99],[375,123],[715,99],[715,298],[1040,298]]:[[165,139],[165,163],[745,139],[745,163],[1040,139]])
const boundaries=computed(()=>props.variant==='demand'?[{name:'room_temp',value:temp(props.measured),x:270,y:0}]:[{name:'room_temp',value:temp(props.measured),x:5,y:125},{name:'cool_setpoint',value:temp(props.setpoint),x:5,y:153}])
const activeTooltip=ref<string|null>(null)
const viewport=ref<HTMLElement|null>(null)
const canPanLeft=ref(false),canPanRight=ref(false)
let pan:{x:number;y:number;left:number;top:number}|null=null
function startPan(event:PointerEvent){if((event.target as HTMLElement).closest('button'))return;const el=viewport.value;if(!el)return;pan={x:event.clientX,y:event.clientY,left:el.scrollLeft,top:el.scrollTop};el.setPointerCapture(event.pointerId)}
function movePan(event:PointerEvent){if(!pan||!viewport.value)return;viewport.value.scrollLeft=pan.left+pan.x-event.clientX;viewport.value.scrollTop=pan.top+pan.y-event.clientY}
function stopPan(){pan=null}
function updateOverflow(){const el=viewport.value;if(!el)return;canPanLeft.value=el.scrollLeft>4;canPanRight.value=el.scrollLeft+el.clientWidth<el.scrollWidth-4}
function panBy(direction:number){viewport.value?.scrollBy({left:direction*380,behavior:'smooth'})}
onMounted(()=>{nextTick(updateOverflow);window.addEventListener('resize',updateOverflow)})
onUnmounted(()=>window.removeEventListener('resize',updateOverflow))
</script>
