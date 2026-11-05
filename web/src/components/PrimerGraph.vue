<template>
  <div class="primer-graph-panel" :class="{'primer-graph-hero':variant==='hero','primer-graph-demand':variant==='demand'}">
    <div v-if="variant!=='hero'" class="primer-graph-toolbar"><span>{{ variant==='demand' ? 'VAV COMPOSITE / COOLING DEMAND' : 'VAV COMPOSITE / COOLING MODE' }}</span><span>LIVE · READ ONLY <span class="primer-pan-hint">· Drag to pan</span></span></div>
    <div ref="viewport" class="primer-graph-viewport" @scroll="updateOverflow" @pointerdown="startPan" @pointermove="movePan" @pointerup="stopPan" @pointercancel="stopPan">
      <div class="graph-canvas primer-graph-canvas" :style="{width:canvasWidth+'px',height:canvasHeight+'px'}">
        <svg class="edge-layer" :width="canvasWidth" :height="canvasHeight" aria-hidden="true">
          <g v-for="(edge,index) in edges" :key="index"><path :d="edge.path" fill="none" :stroke="signalColor(edge.unit)" stroke-width="2.5" opacity=".9"/><circle :cx="edge.end.x" :cy="edge.end.y" r="3" :fill="signalColor(edge.unit)"/></g>
        </svg>
        <BrickPoint v-for="boundary in boundaries" :key="boundary.name" :direction="boundary.direction" :name="boundary.name" :label="boundary.label" :description="boundary.description" :data-type="boundary.dataType" :brick-class="boundary.brickClass" :unit="boundary.unit" :value="boundary.value" :generic="boundary.brickClass==='Point'" :anchor-id="`boundary:${boundary.name}`" :style="{left:boundary.x+'px',top:boundary.y+'px'}"/>
        <article v-for="node in nodes" :key="node.name" class="graph-node primer-graph-node" :style="{left:node.x+'px',top:node.y+'px'}" @mouseenter="activeTooltip=node.name" @mouseleave="activeTooltip=null">
          <div class="node-head"><span>{{ node.name }}</span><span class="node-kind">{{ node.kind }}</span><button class="node-info" type="button" :aria-label="`About ${node.name}`" @click.stop="activeTooltip=activeTooltip===node.name?null:node.name">i</button></div>
          <div class="node-ports"><div class="input-ports"><div v-for="port in node.inputs" :key="port.name" class="port-row"><i class="port-dot input-dot" :data-anchor="`${node.name}.${port.name}`"/><span>{{ port.name }} <b>{{ port.value }}</b></span></div></div><div class="output-ports"><div v-for="port in node.outputs" :key="port.name" class="port-row"><span>{{ port.name }} <b>{{ port.value }}</b></span><i class="port-dot output-dot" :data-anchor="`${node.name}.${port.name}`"/></div></div></div>
          <div v-if="node.setting" class="primer-node-setting">{{ node.setting }}</div>
          <div v-if="activeTooltip===node.name" class="node-tooltip" :class="{'tooltip-left':node.x>650}" role="tooltip"><strong>{{ node.name }}</strong><span class="tooltip-kind">{{ node.kind }}</span><p>{{ node.description }}</p><div class="tooltip-group"><b>LIVE INPUTS</b><div v-for="port in node.inputs" :key="port.name" class="tooltip-port">{{ port.name }} · {{ port.value }}</div></div><div class="tooltip-group"><b>LIVE OUTPUTS</b><div v-for="port in node.outputs" :key="port.name" class="tooltip-port">{{ port.name }} · {{ port.value }}</div></div></div>
        </article>
      </div>
    </div>
    <button v-if="canPanLeft" class="primer-graph-pan primer-graph-pan-left" type="button" aria-label="More primer graph to the left" @click="panBy(-1)">←</button>
    <button v-if="canPanRight" class="primer-graph-pan primer-graph-pan-right" type="button" aria-label="More primer graph to the right" @click="panBy(1)">→</button>
    <div v-if="variant!=='hero'" class="primer-graph-caption"><span>{{ variant==='demand' ? 'The same cool_error → cool_demand_gain → cool_demand_limit path is inside each VAV in the editor.' : 'The Boolean cooling_mode output turns true only when cooling error exceeds the 0.5 K deadband.' }}</span><strong>{{ variant==='demand' ? `cooling_demand = ${demand.toFixed(2)}` : `cooling_mode = ${coolingMode}` }}</strong></div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { blockCatalog } from '../lib/blockCatalog'
import { orthogonalWire } from '../lib/graphRouting'
import { signalColor } from '../lib/signalUnits'
import BrickPoint from './BrickPoint.vue'

interface Port { name:string; value:string }
interface Node { name:string; kind:string; x:number; y:number; inputs:Port[]; outputs:Port[]; setting?:string; description:string }
interface Point { x:number; y:number }
interface Edge { path:string; end:Point; unit:string }
interface Boundary { name:string; value:string; x:number; y:number; brickClass:string; unit:string; direction:'input'|'output'; label?:string; description?:string; dataType?:string }
const props=defineProps<{variant:'hero'|'demand'|'mode'; measured:number; setpoint:number; gain:number}>()
const error=computed(()=>props.measured-props.setpoint)
const scaled=computed(()=>error.value*props.gain)
const demand=computed(()=>Math.max(0,Math.min(1,scaled.value)))
const coolingMode=computed(()=>error.value>0.5)
const description=(kind:string)=>blockCatalog.find(item=>item.type===kind)?.description??''
const temp=(value:number)=>`${value.toFixed(1)} °C`
const difference=(value:number)=>`${value.toFixed(1)} K`
const fraction=(value:number)=>value.toFixed(2)
const heroWidth=ref(530)
const canvasWidth=computed(()=>props.variant==='hero'?Math.max(680,heroWidth.value):props.variant==='demand'?1380:1330)
const canvasHeight=computed(()=>props.variant==='hero'?380:props.variant==='demand'?480:420)
const nodes=computed<Node[]>(()=>props.variant==='hero'?[
  {name:'cool_setpoint',kind:'Constant',x:20,y:155,inputs:[],outputs:[{name:'y',value:temp(props.setpoint)}],setting:`k = ${temp(props.setpoint)}`,description:description('Constant')},
  {name:'cool_error',kind:'Subtract',x:350,y:85,inputs:[{name:'u1',value:temp(props.measured)},{name:'u2',value:temp(props.setpoint)}],outputs:[{name:'y',value:difference(error.value)}],description:description('Subtract')},
]:props.variant==='demand'?[
  {name:'cool_setpoint',kind:'Constant',x:20,y:250,inputs:[],outputs:[{name:'y',value:temp(props.setpoint)}],setting:`k = ${temp(props.setpoint)}`,description:description('Constant')},
  {name:'cool_error',kind:'Subtract',x:350,y:60,inputs:[{name:'u1',value:temp(props.measured)},{name:'u2',value:temp(props.setpoint)}],outputs:[{name:'y',value:difference(error.value)}],description:description('Subtract')},
  {name:'cool_demand_gain',kind:'MultiplyByParameter',x:720,y:60,inputs:[{name:'u',value:difference(error.value)}],outputs:[{name:'y',value:fraction(scaled.value)}],setting:`k = ${props.gain.toFixed(2)} K⁻¹`,description:description('MultiplyByParameter')},
  {name:'cool_demand_limit',kind:'Limiter',x:720,y:280,inputs:[{name:'u',value:fraction(scaled.value)}],outputs:[{name:'y',value:fraction(demand.value)}],setting:'uMin = 0 · uMax = 1',description:description('Limiter')},
]:[
  {name:'cool_setpoint',kind:'Constant',x:20,y:210,inputs:[],outputs:[{name:'y',value:temp(props.setpoint)}],setting:`k = ${temp(props.setpoint)}`,description:description('Constant')},
  {name:'cool_error',kind:'Subtract',x:350,y:30,inputs:[{name:'u1',value:temp(props.measured)},{name:'u2',value:temp(props.setpoint)}],outputs:[{name:'y',value:difference(error.value)}],description:description('Subtract')},
  {name:'cool_deadband',kind:'Constant',x:350,y:230,inputs:[],outputs:[{name:'y',value:'0.5 K'}],setting:'k = 0.5 K',description:description('Constant')},
  {name:'cooling_mode',kind:'Greater',x:750,y:30,inputs:[{name:'u1',value:difference(error.value)},{name:'u2',value:'0.5 K'}],outputs:[{name:'y',value:String(coolingMode.value)}],description:description('Greater')},
])
const boundaries=computed<Boundary[]>(()=>{
  const room:Boundary={name:'room_temp',label:'Zone air temperature',description:'Measured zone temperature entering the VAV cooling logic.',value:temp(props.measured),x:20,y:props.variant==='hero'?35:40,brickClass:'Zone_Air_Temperature_Sensor',unit:'°C',dataType:'Real',direction:'input'}
  if(props.variant==='demand')return [room,{name:'cooling_demand',label:'Cooling demand',description:'Normalized diagnostic cooling request calculated by the VAV controller.',value:fraction(demand.value),x:1110,y:317.3,brickClass:'Point',unit:'fraction (0–1)',dataType:'Real',direction:'output'}]
  if(props.variant==='mode')return [room,{name:'cooling_mode',label:'Cooling mode',description:'True when zone temperature exceeds the cooling setpoint by more than the deadband.',value:String(coolingMode.value),x:1080,y:67.3,brickClass:'Point',unit:'unitless',dataType:'Boolean',direction:'output'}]
  return [room]
})
const activeTooltip=ref<string|null>(null)
const viewport=ref<HTMLElement|null>(null)
const anchors=ref<Record<string,Point>>({})
const canPanLeft=ref(false),canPanRight=ref(false)
let observer:ResizeObserver|null=null
let pan:{x:number;y:number;left:number;top:number}|null=null
function measureAnchors(){
  const canvas=viewport.value?.querySelector('.primer-graph-canvas')
  if(!canvas)return
  const origin=canvas.getBoundingClientRect(),scaleX=origin.width/canvas.clientWidth,scaleY=origin.height/canvas.clientHeight,found:Record<string,Point>={}
  canvas.querySelectorAll<HTMLElement>('[data-anchor]').forEach(element=>{
    const box=element.getBoundingClientRect(),id=element.dataset.anchor!
    found[id]={x:(box.left+box.width/2-origin.left)/scaleX,y:(box.top+box.height/2-origin.top)/scaleY}
  })
  anchors.value=found
}
const edges=computed<Edge[]>(()=>{
  const point=(id:string)=>anchors.value[id]
  const result:Edge[]=[]
  const add=(from:string,to:string,unit:string)=>{const a=point(from),b=point(to);if(a&&b)result.push({path:orthogonalWire(a,b),end:b,unit})}
  if(props.variant==='hero'){
    add('boundary:room_temp','cool_error.u1','°C')
    add('cool_setpoint.y','cool_error.u2','°C')
    return result
  }
  if(props.variant==='demand'){
    add('boundary:room_temp','cool_error.u1','°C')
    add('cool_setpoint.y','cool_error.u2','°C')
    add('cool_error.y','cool_demand_gain.u','K')
    add('cool_demand_gain.y','cool_demand_limit.u','fraction (0–1)')
    add('cool_demand_limit.y','boundary:cooling_demand','fraction (0–1)')
  }else{
    add('boundary:room_temp','cool_error.u1','°C')
    add('cool_setpoint.y','cool_error.u2','°C')
    add('cool_error.y','cooling_mode.u1','K')
    add('cool_deadband.y','cooling_mode.u2','K')
    add('cooling_mode.y','boundary:cooling_mode','Boolean')
  }
  return result
})
function startPan(event:PointerEvent){if((event.target as HTMLElement).closest('button'))return;const el=viewport.value;if(!el)return;pan={x:event.clientX,y:event.clientY,left:el.scrollLeft,top:el.scrollTop};el.setPointerCapture(event.pointerId)}
function movePan(event:PointerEvent){if(!pan||!viewport.value)return;viewport.value.scrollLeft=pan.left+pan.x-event.clientX;viewport.value.scrollTop=pan.top+pan.y-event.clientY}
function stopPan(){pan=null}
function updateOverflow(){const el=viewport.value;if(!el)return;canPanLeft.value=el.scrollLeft>4;canPanRight.value=el.scrollLeft+el.clientWidth<el.scrollWidth-4}
function panBy(direction:number){viewport.value?.scrollBy({left:direction*380,behavior:'smooth'})}
function refreshGeometry(){nextTick(()=>{measureAnchors();updateOverflow()})}
onMounted(()=>{if(viewport.value){observer=new ResizeObserver(()=>{if(props.variant==='hero')heroWidth.value=Math.max(420,viewport.value?.clientWidth??530);refreshGeometry()});observer.observe(viewport.value)}refreshGeometry();document.fonts.ready.then(refreshGeometry);window.addEventListener('resize',refreshGeometry)})
onUnmounted(()=>{observer?.disconnect();window.removeEventListener('resize',refreshGeometry)})
watch(()=>[props.measured,props.setpoint,props.gain],refreshGeometry)
</script>
