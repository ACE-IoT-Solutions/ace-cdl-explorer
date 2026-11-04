<template>
  <div class="primer-graph-panel">
    <div class="primer-graph-toolbar"><span>{{ variant==='demand' ? 'VAV COMPOSITE / COOLING DEMAND' : 'VAV COMPOSITE / COOLING MODE' }}</span><span>LIVE · READ ONLY <span class="primer-pan-hint">· Drag to pan</span></span></div>
    <div ref="viewport" class="primer-graph-viewport" @scroll="updateOverflow" @pointerdown="startPan" @pointermove="movePan" @pointerup="stopPan" @pointercancel="stopPan">
      <div class="graph-canvas primer-graph-canvas" :style="{width:'1060px',height:'420px'}">
        <svg class="edge-layer" width="1060" height="420" aria-hidden="true">
          <g v-for="(edge,index) in edges" :key="index"><path :d="edge.path" fill="none" stroke="var(--ace-secondary)" stroke-width="2" opacity=".9"/><circle :cx="edge.end.x" :cy="edge.end.y" r="3" fill="var(--ace-secondary)"/></g>
        </svg>
        <div v-for="boundary in boundaries" :key="boundary.name" :data-anchor="`boundary:${boundary.name}`" class="primer-boundary" :style="{left:boundary.x+'px',top:boundary.y+'px'}"><span>{{ boundary.name }}</span><b>{{ boundary.value }}</b></div>
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
    <div class="primer-graph-caption"><span>{{ variant==='demand' ? 'The same cool_error → cool_demand_gain → cool_demand_limit path is inside each VAV in the editor.' : 'The Boolean cooling_mode output turns true only when cooling error exceeds the 0.5 K deadband.' }}</span><strong>{{ variant==='demand' ? `cooling_demand = ${demand.toFixed(2)}` : `cooling_mode = ${coolingMode}` }}</strong></div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { blockCatalog } from '../lib/blockCatalog'

interface Port { name:string; value:string }
interface Node { name:string; kind:string; x:number; y:number; inputs:Port[]; outputs:Port[]; setting?:string; description:string }
interface Point { x:number; y:number }
interface Edge { path:string; end:Point }
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
  {name:'cool_deadband',kind:'Constant',x:380,y:230,inputs:[],outputs:[{name:'y',value:'0.5 K'}],setting:'k = 0.5 K',description:description('Constant')},
  {name:'cooling_mode',kind:'Greater',x:740,y:70,inputs:[{name:'u1',value:difference(error.value)},{name:'u2',value:'0.5 K'}],outputs:[{name:'y',value:String(coolingMode.value)}],description:description('Greater')},
])
const boundaries=computed(()=>props.variant==='demand'?[{name:'room_temp',value:temp(props.measured),x:270,y:0}]:[{name:'room_temp',value:temp(props.measured),x:5,y:125},{name:'cool_setpoint',value:temp(props.setpoint),x:5,y:153}])
const activeTooltip=ref<string|null>(null)
const viewport=ref<HTMLElement|null>(null)
const anchors=ref<Record<string,Point>>({})
const canPanLeft=ref(false),canPanRight=ref(false)
let pan:{x:number;y:number;left:number;top:number}|null=null
function measureAnchors(){
  const canvas=viewport.value?.querySelector('.primer-graph-canvas')
  if(!canvas)return
  const origin=canvas.getBoundingClientRect(),scaleX=origin.width/canvas.clientWidth,scaleY=origin.height/canvas.clientHeight,found:Record<string,Point>={}
  canvas.querySelectorAll<HTMLElement>('[data-anchor]').forEach(element=>{
    const box=element.getBoundingClientRect(),id=element.dataset.anchor!
    found[id]=id.startsWith('boundary:')
      ? props.variant==='demand' ? {x:(box.left+box.width/2-origin.left)/scaleX,y:(box.bottom-origin.top)/scaleY} : {x:(box.right-origin.left)/scaleX,y:(box.top+box.height/2-origin.top)/scaleY}
      : {x:(box.left+box.width/2-origin.left)/scaleX,y:(box.top+box.height/2-origin.top)/scaleY}
  })
  anchors.value=found
}
function curve(a:Point,b:Point):string{const bend=Math.max(28,Math.min(90,(b.x-a.x)/2));return `M${a.x} ${a.y} C${a.x+bend} ${a.y},${b.x-bend} ${b.y},${b.x} ${b.y}`}
function roundedUp(a:Point,b:Point):string{
  const mid=(a.x+b.x)/2,r=Math.min(14,(b.x-a.x)/4,(a.y-b.y)/4)
  return `M${a.x} ${a.y} L${mid-r} ${a.y} Q${mid} ${a.y} ${mid} ${a.y-r} L${mid} ${b.y+r} Q${mid} ${b.y} ${mid+r} ${b.y} L${b.x} ${b.y}`
}
function aroundBottom(a:Point,b:Point):string{
  const right=a.x+38,left=b.x-25,midY=b.y-94,r=14
  return `M${a.x} ${a.y} L${right-r} ${a.y} Q${right} ${a.y} ${right} ${a.y+r} L${right} ${midY-r} Q${right} ${midY} ${right-r} ${midY} L${left+r} ${midY} Q${left} ${midY} ${left} ${midY+r} L${left} ${b.y-r} Q${left} ${b.y} ${left+r} ${b.y} L${b.x} ${b.y}`
}
const edges=computed<Edge[]>(()=>{
  const point=(id:string)=>anchors.value[id]
  const result:Edge[]=[]
  const add=(from:string,to:string,route:'curve'|'up'|'around'='curve')=>{
    const a=point(from),b=point(to);if(!a||!b)return
    result.push({path:route==='up'?roundedUp(a,b):route==='around'?aroundBottom(a,b):curve(a,b),end:b})
  }
  const stub=(from:string)=>{const a=point(from);if(!a)return;const b={x:1040,y:a.y};result.push({path:`M${a.x} ${a.y} L${b.x} ${b.y}`,end:b})}
  if(props.variant==='demand'){
    const boundary=point('boundary:room_temp'),input=point('cool_error.u1')
    if(boundary&&input){const gapX=input.x-36;result.push({path:`M${boundary.x} ${boundary.y} L${gapX} ${boundary.y} L${gapX} ${input.y} L${input.x} ${input.y}`,end:input})}
    add('cool_setpoint.y','cool_error.u2')
    add('cool_error.y','cool_demand_gain.u')
    add('cool_demand_gain.y','cool_demand_limit.u','around')
    stub('cool_demand_limit.y')
  }else{
    add('boundary:room_temp','cool_error.u1')
    add('boundary:cool_setpoint','cool_error.u2')
    add('cool_error.y','cooling_mode.u1')
    add('cool_deadband.y','cooling_mode.u2','up')
    stub('cooling_mode.y')
  }
  return result
})
function startPan(event:PointerEvent){if((event.target as HTMLElement).closest('button'))return;const el=viewport.value;if(!el)return;pan={x:event.clientX,y:event.clientY,left:el.scrollLeft,top:el.scrollTop};el.setPointerCapture(event.pointerId)}
function movePan(event:PointerEvent){if(!pan||!viewport.value)return;viewport.value.scrollLeft=pan.left+pan.x-event.clientX;viewport.value.scrollTop=pan.top+pan.y-event.clientY}
function stopPan(){pan=null}
function updateOverflow(){const el=viewport.value;if(!el)return;canPanLeft.value=el.scrollLeft>4;canPanRight.value=el.scrollLeft+el.clientWidth<el.scrollWidth-4}
function panBy(direction:number){viewport.value?.scrollBy({left:direction*380,behavior:'smooth'})}
function refreshGeometry(){nextTick(()=>{measureAnchors();updateOverflow()})}
onMounted(()=>{refreshGeometry();document.fonts.ready.then(refreshGeometry);window.addEventListener('resize',refreshGeometry)})
onUnmounted(()=>window.removeEventListener('resize',refreshGeometry))
watch(()=>[props.measured,props.setpoint,props.gain],refreshGeometry)
</script>
