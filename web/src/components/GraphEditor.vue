<template>
  <div v-if="path.length" class="nested-backdrop" @click="path=[]" />
  <div class="editor-shell" :class="{ 'nested-mode': path.length }" :role="path.length ? 'dialog' : undefined" :aria-modal="path.length ? 'true' : undefined" :aria-label="path.length ? `Editing ${graph.name}` : undefined">
    <aside class="editor-side">
      <div class="panel-title"><span class="eyebrow">BLOCK LIBRARY</span><h2>Build your logic</h2></div>
      <p class="muted small">Choose a block, then connect an output port to an input port.</p>
      <div class="palette-list"><button v-for="item in palette" :key="item.type" class="palette-item" @click="addBlock(item)"><span class="palette-glyph" aria-hidden="true">＋</span><span>{{ item.label }}</span></button></div>
    </aside>

    <section class="editor-main">
      <div class="canvas-toolbar">
        <div class="breadcrumbs"><button @click="path=[]">{{ project.graph.name }}</button><template v-for="(part,i) in path" :key="i"><span>›</span><button @click="path=path.slice(0,i+1)">{{ part }}</button></template></div>
        <div class="toolbar-actions"><span v-if="pending" class="connect-hint">Connect {{ pending.node }}.{{ pending.port }} → choose input</span><button v-if="pending" class="text-button" @click="pending=null">Cancel</button><span class="pan-hint">Drag background to pan</span><span>{{ graph.blocks?.length ?? 0 }} blocks</span><button type="button" class="text-button" aria-label="Arrange blocks by signal flow" @click="arrangeGraph">Arrange</button><div class="zoom-controls" aria-label="Canvas zoom"><button type="button" aria-label="Zoom out" :disabled="zoom<=0.5" @click="setZoom(zoom-0.25)">−</button><button type="button" class="zoom-level" aria-label="Reset zoom to 100 percent" @click="setZoom(1)">{{ Math.round(zoom*100) }}%</button><button type="button" aria-label="Zoom in" :disabled="zoom>=2" @click="setZoom(zoom+0.25)">+</button></div><button v-if="path.length" class="modal-close" aria-label="Close nested editor" @click="path=[]">×</button></div>
      </div>
      <div class="canvas-viewport"><div v-if="hasBoundaryEdges" class="boundary-legend">{{ brickBoundary ? 'Brick point cards are simulation inputs and outputs; brick:Point marks a modeled value. CDL blocks perform the calculations.' : 'IN / OUT mark ports of this control graph.' }}</div><div class="signal-legend" aria-label="Signal route colors"><span v-for="unit in visibleUnits" :key="unit"><i :style="{background:signalColor(unit)}"/>{{ unit }}</span></div><div ref="canvasScroll" class="canvas-scroll" @scroll="updateOverflow">
        <div class="canvas-sizer" :style="{ width: canvasWidth*zoom+'px', height: canvasHeight*zoom+'px' }"><div class="graph-canvas" :class="{ panning }" :style="{ width: canvasWidth+'px', height: canvasHeight+'px', transform:`scale(${zoom})` }" @pointerdown="startPan">
          <svg class="edge-layer" :width="canvasWidth" :height="canvasHeight" aria-hidden="true">
            <g v-for="(edge,index) in visibleEdges" :key="index"><path :d="edge.path" fill="none" :stroke="signalColor(edge.unit)" stroke-width="2.5" opacity=".9"/><circle v-if="!edge.terminal" :cx="edge.x" :cy="edge.y" r="3" :fill="signalColor(edge.unit)"/><g v-if="edge.terminal" class="edge-terminal"><title>{{ edge.terminal.title }} · {{ edge.unit }}</title><rect :x="edge.terminal.x" :y="edge.terminal.y" width="28" height="18" rx="4"/><text :x="edge.terminal.x+14" :y="edge.terminal.y+12">{{ edge.terminal.direction }}</text></g></g>
            <g v-for="(bridge,index) in bridges" :key="`bridge-${index}`" class="wire-bridge"><circle :cx="bridge.x" :cy="bridge.y" r="7" fill="#171b24"/><path :d="`M${bridge.x} ${bridge.y-7} C${bridge.x+8} ${bridge.y-7} ${bridge.x+8} ${bridge.y+7} ${bridge.x} ${bridge.y+7}`" fill="none" :stroke="bridge.color" stroke-width="2.5"/></g>
          </svg>
          <BrickPoint v-for="point in boundaryCards" :key="point.key" compact :direction="point.direction" :name="point.port.name" :label="point.port.label" :description="point.port.description" :data-type="point.port.type" :brick-class="point.port.brick_class ?? 'Point'" :unit="point.port.unit" :generic="!point.port.brick_class || point.port.brick_class==='Point'" :style="{left:point.x+'px',top:point.y+'px'}"/>
          <article v-for="child in graph.blocks ?? []" :key="child.name" class="graph-node" :class="{ selected: selected===child.name, 'has-tooltip':activeTooltip===child.name }" :style="position(child.name)" @click="selected=child.name;activeTooltip=null" @mouseenter="activeTooltip=child.name" @mouseleave="!isMobile && (activeTooltip=null)" @focusin="activeTooltip=child.name" @focusout="hideTooltipOnBlur($event,child.name)">
            <div class="node-head" @pointerdown="startDrag($event,child.name)"><span>{{ child.name }}</span><span class="node-kind">{{ shortType(child.block_type) }}</span><button class="node-info" type="button" :aria-label="`About ${child.name}`" :aria-describedby="activeTooltip===child.name ? tooltipId(child.name) : undefined" @pointerdown.stop @click.stop="activeTooltip=child.name">i</button></div>
            <div class="node-ports">
              <div class="input-ports"><button v-for="port in child.inputs ?? []" :key="port.name" class="port-row" :title="`${port.label ?? port.name} · ${port.brick_class ? `brick:${port.brick_class}` : 'calculated signal'} · ${port.unit ?? port.type}`" @click.stop="chooseInput(child.name,port.name)"><i class="port-dot input-dot"/>{{ port.name }}</button></div>
              <div class="output-ports"><button v-for="port in child.outputs ?? []" :key="port.name" class="port-row" :title="`${port.label ?? port.name} · ${port.brick_class ? `brick:${port.brick_class}` : 'calculated signal'} · ${port.unit ?? port.type}`" @click.stop="pending={node:child.name,port:port.name}">{{ port.name }}<i class="port-dot output-dot"/></button></div>
            </div>
            <button v-if="child.category==='composite' || child.blocks?.length" class="node-open" @click.stop="path=[...path,child.name];selected=null">Open composite ↗</button>
            <Teleport to="body" :disabled="!isMobile"><div v-if="activeTooltip===child.name" :id="tooltipId(child.name)" class="node-tooltip" :class="{ 'tooltip-left':displayX(child.name)>canvasWidth-420 }" role="tooltip">
              <button v-if="isMobile" class="tooltip-close" type="button" aria-label="Close block information" @click.stop="activeTooltip=null">×</button>
              <strong>{{ child.name }}</strong><span class="tooltip-kind">{{ shortType(child.block_type) }}{{ child.category==='composite' ? ' · composite' : '' }}</span>
              <p>{{ blockDescription(child) }}</p>
              <div v-if="child.notes?.trim()" class="tooltip-notes"><b>EXAMPLE NOTE</b><span>{{ child.notes }}</span></div>
              <div class="tooltip-group"><b>INPUTS · {{ child.inputs?.length ?? 0 }}</b><div v-if="!child.inputs?.length" class="tooltip-empty">No input ports</div><div v-for="port in child.inputs ?? []" :key="`in-${port.name}`" class="tooltip-port"><span>{{ port.label ?? port.name }} <small>{{ port.type }}{{ port.unit ? ` · ${port.unit}` : '' }}</small></span><em>{{ port.brick_class ? `brick:${port.brick_class}` : portDescription(child,'input',port) }}</em></div></div>
              <div class="tooltip-group"><b>OUTPUTS · {{ child.outputs?.length ?? 0 }}</b><div v-if="!child.outputs?.length" class="tooltip-empty">No output ports</div><div v-for="port in child.outputs ?? []" :key="`out-${port.name}`" class="tooltip-port"><span>{{ port.label ?? port.name }} <small>{{ port.type }}{{ port.unit ? ` · ${port.unit}` : '' }}</small></span><em>{{ port.brick_class ? `brick:${port.brick_class}` : portDescription(child,'output',port) }}</em></div></div>
            </div></Teleport>
          </article>
          <div v-if="!graph.blocks?.length" class="empty-canvas">Add a block from the library to begin.</div>
        </div></div>
      </div>
      <button v-if="overflow.left" class="pan-indicator pan-left" type="button" aria-label="More graph content to the left" @click="panBy(-1,0)">←</button>
      <button v-if="overflow.right" class="pan-indicator pan-right" type="button" aria-label="More graph content to the right" @click="panBy(1,0)">→</button>
      <button v-if="overflow.top" class="pan-indicator pan-top" type="button" aria-label="More graph content above" @click="panBy(0,-1)">↑</button>
      <button v-if="overflow.bottom" class="pan-indicator pan-bottom" type="button" aria-label="More graph content below" @click="panBy(0,1)">↓</button></div>
    </section>

    <aside class="editor-side inspector">
      <div class="panel-title"><span class="eyebrow">INSPECTOR</span><h2>{{ selectedBlock?.name ?? 'Graph details' }}</h2></div>
      <template v-if="selectedBlock">
        <p class="muted small">{{ selectedBlock.description || selectedBlock.block_type }}</p>
        <label class="field"><span>Block name</span><input :value="selectedBlock.name" @change="renameBlock(($event.target as HTMLInputElement).value)"/></label>
        <label class="field"><span>Notes / comments</span><textarea :key="`${prefix}${selectedBlock.name}`" :value="selectedBlock.notes ?? ''" rows="5" placeholder="Explain this block’s role in the control sequence…" @change="setNotes(($event.target as HTMLTextAreaElement).value)"/></label>
        <div class="section-label">Parameters</div>
        <p v-if="!selectedBlock.parameters?.length" class="muted small">No parameters</p>
        <label v-for="p in selectedBlock.parameters ?? []" :key="p.name" class="field"><span>{{ p.name }} <small>{{ p.unit }}</small></span><select v-if="typeof p.value === 'boolean'" :value="String(p.value)" @change="setParameter(p.name,($event.target as HTMLSelectElement).value==='true')"><option value="true">True</option><option value="false">False</option></select><input v-else type="number" step="any" :value="p.value" @change="setParameter(p.name,Number(($event.target as HTMLInputElement).value))"/></label>
        <template v-if="selectedBlock.category==='composite'"><div class="section-label">Composite boundary</div><p class="muted small">{{ selectedBlock.inputs?.length ?? 0 }} inputs · {{ selectedBlock.outputs?.length ?? 0 }} outputs</p><label class="field"><span>New port name</span><input v-model="newPortName" placeholder="e.g. room_temp"/></label><label class="field"><span>Direction</span><select v-model="newPortDirection"><option value="input">Input</option><option value="output">Output</option></select></label><label class="field"><span>Signal type</span><select v-model="newPortType"><option>Real</option><option>Boolean</option><option>Integer</option><option>String</option></select></label><button class="outline-button" @click="addPort">Add port</button></template>
        <button class="danger-button" @click="removeBlock">Remove block</button>
      </template>
      <template v-else>
        <p class="muted small">Select a block to edit its parameters. Drag a header to reposition it.</p>
        <label class="field"><span>Graph notes / comments</span><textarea :key="`${prefix}graph-notes`" :value="graph.notes ?? ''" rows="4" placeholder="Explain what this control graph does…" @change="setGraphNotes(($event.target as HTMLTextAreaElement).value)"/></label>
        <div class="section-label">Graph connections</div>
        <div class="edge-list"><div v-for="(edge,index) in graph.connections ?? []" :key="index" class="edge-item"><span>{{ edgeEndpoints(edge).join(' → ') }}</span><button :aria-label="`Remove connection ${index+1}`" @click="removeEdge(index)">×</button></div></div>
        <div class="section-label">Connect boundary ports</div>
        <label class="field"><span>From</span><select v-model="boundarySource"><option value="">Select output</option><option v-for="port in graph.inputs ?? []" :key="port.name" :value="port.name">Input: {{ port.label ?? port.name }}</option><template v-for="child in graph.blocks ?? []" :key="child.name"><option v-for="port in child.outputs ?? []" :key="port.name" :value="`${child.name}.${port.name}`">{{ child.name }} · {{ port.label ?? port.name }}</option></template></select></label>
        <label class="field"><span>To</span><select v-model="boundaryTarget"><option value="">Select input</option><option v-for="port in graph.outputs ?? []" :key="port.name" :value="port.name">Output: {{ port.label ?? port.name }}</option><template v-for="child in graph.blocks ?? []" :key="child.name"><option v-for="port in child.inputs ?? []" :key="port.name" :value="`${child.name}.${port.name}`">{{ child.name }} · {{ port.label ?? port.name }}</option></template></select></label>
        <button class="solid-button" :disabled="!boundarySource || !boundaryTarget" @click="connectBoundary">Add connection</button>
      </template>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { edgeEndpoints, layoutFor, palette, type Block, type Connector, type Project } from '../lib/types'
import { blockDescription, portDescription } from '../lib/blockHelp'
import { routeWirePoints, roundedWire, wireBridges, type GraphBox, type GraphPoint, type RoutedTrack } from '../lib/graphRouting'
import { resolveSignalUnits, signalColor } from '../lib/signalUnits'
import BrickPoint from './BrickPoint.vue'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ change: [project: Project] }>()
const path = ref<string[]>([])
const selected = ref<string | null>(null)
const pending = ref<{node:string;port:string}|null>(null)
const boundarySource = ref('')
const boundaryTarget = ref('')
const newPortName = ref('')
const newPortDirection = ref<'input'|'output'>('input')
const newPortType = ref<Connector['type']>('Real')
const canvasScroll = ref<HTMLElement|null>(null)
const activeTooltip = ref<string|null>(null)
const isMobile = ref(typeof window!=='undefined'&&window.innerWidth<=820)
const zoom = ref(1)
const panning = ref(false)
const overflow = ref({left:false,right:false,top:false,bottom:false})
let canvasObserver:ResizeObserver|null=null
const graph = computed(() => { let g=props.project.graph; for (const name of path.value) {g=(g.blocks ?? []).find(b=>b.name===name) ?? g} return g })
const selectedBlock = computed(() => (graph.value.blocks ?? []).find(b=>b.name===selected.value))
const prefix = computed(() => path.value.length ? `${path.value.join('.')}.` : '')
const brickBoundary = computed(() => ['BuildingController','VAVBoxController','AHUController','PlantDispatch'].includes(graph.value.block_type) || [...(graph.value.inputs??[]),...(graph.value.outputs??[])].some(port=>port.brick_class))
const columnScale = computed(()=>1)
const columnOffset = computed(()=>brickBoundary.value?240:0)
function displayX(name:string){const x=props.project.layout[`${prefix.value}${name}`]?.x??60;return x*columnScale.value+columnOffset.value}
const canvasWidth = computed(() => Math.max(900,...(graph.value.blocks ?? []).map(b => displayX(b.name)+390),...boundaryCards.value.map(card=>card.x+240)))
const canvasHeight = computed(() => Math.max(660,...(graph.value.blocks ?? []).map(b => (props.project.layout[`${prefix.value}${b.name}`]?.y ?? 0)+350),...boundaryCards.value.map(card=>card.y+80)))
function position(name: string) {const p=props.project.layout[`${prefix.value}${name}`] ?? {x:60,y:80};return {left:`${displayX(name)}px`,top:`${p.y}px`}}
function shortType(type: string) {return type.split('.').at(-1) ?? type}
function tooltipId(name:string){return `help-${[...path.value,name].join('-').replace(/[^A-Za-z0-9_-]/g,'-')}`}
function hideTooltipOnBlur(event:FocusEvent,name:string){const node=event.currentTarget as HTMLElement;if(!node.contains(event.relatedTarget as Node)&&activeTooltip.value===name)activeTooltip.value=null}
function edit(fn:(graph:Block,project:Project)=>void) {const project=structuredClone(props.project);let graph=project.graph;for(const name of path.value){graph=(graph.blocks ?? []).find(b=>b.name===name)!}fn(graph,project);emit('change',project)}
function arrangeGraph(){
  const keys=(graph.value.blocks??[]).map(block=>`${prefix.value}${block.name}`)
  edit((_,project)=>{const arranged=layoutFor(project.graph);for(const key of keys)if(arranged[key])project.layout[key]=arranged[key]})
  canvasScroll.value?.scrollTo(0,0)
}
async function addBlock(item: typeof palette[number]) {
  let template: Block | null = null
  if(item.type==='VAVBoxController'){
    const response=await fetch(new URL('examples/vav_composite_template.json',document.baseURI))
    if(!response.ok)return
    template=await response.json() as Block
  }
  edit((g,p)=>{
    const used=new Set((g.blocks ?? []).map(b=>b.name))
    let n=1;const stem=item.label.replace(/\s/g,'')
    while(used.has(`${stem}${n}`))n++
    const name=`${stem}${n}`
    const child:Block=template ? structuredClone(template) : {name,block_type:item.type,category:item.type==='Composite'?'composite':'elementary',inputs:structuredClone(item.inputs),outputs:structuredClone(item.outputs),parameters:structuredClone(item.parameters),blocks:item.type==='Composite'?[]:undefined,connections:item.type==='Composite'?[]:undefined,equations:[]}
    child.name=name
    ;(g.blocks??=[]).push(child)
    p.layout[`${prefix.value}${name}`]={x:100+(g.blocks.length%4)*90,y:100+(g.blocks.length%4)*70}
    ;(child.blocks??[]).forEach((b,i)=>{p.layout[`${prefix.value}${name}.${b.name}`]={x:60+(i%5)*250,y:80+Math.floor(i/5)*270}})
    selected.value=name
  })
}
function addPort(){const name=newPortName.value.trim();if(!name||!selected.value||!/^[A-Za-z][A-Za-z0-9_]*$/.test(name))return;edit(g=>{const b=g.blocks?.find(b=>b.name===selected.value);if(!b||[...(b.inputs??[]),...(b.outputs??[])].some(p=>p.name===name))return;const key=newPortDirection.value==='input'?'inputs':'outputs';(b[key]??=[]).push({name,type:newPortType.value,...(b.category==='composite'?{brick_class:'Point'}:{})})});newPortName.value=''}
function removeBlock(){if(!selected.value)return;const name=selected.value;edit(g=>{g.blocks=(g.blocks??[]).filter(b=>b.name!==name);g.connections=(g.connections??[]).filter(e=>{const [a,b]=edgeEndpoints(e);return a.split('.')[0]!==name&&b.split('.')[0]!==name})});selected.value=null}
function renameBlock(name:string){if(!selected.value||!name.trim()||graph.value.blocks?.some(b=>b.name===name&&b.name!==selected.value))return;const old=selected.value;edit((g,p)=>{const b=g.blocks!.find(b=>b.name===old)!;b.name=name;for(const e of g.connections??[]){const [a,z]=edgeEndpoints(e);if(a.startsWith(`${old}.`)){e.from=`${name}.${a.split('.').slice(1).join('.')}`;delete e.from_block;delete e.from_output}if(z.startsWith(`${old}.`)){e.to=`${name}.${z.split('.').slice(1).join('.')}`;delete e.to_block;delete e.to_input}}p.layout[`${prefix.value}${name}`]=p.layout[`${prefix.value}${old}`];delete p.layout[`${prefix.value}${old}`]});selected.value=name}
function setParameter(name:string,value:number|boolean){edit(g=>{const p=g.blocks?.find(b=>b.name===selected.value)?.parameters?.find(p=>p.name===name);if(p)p.value=value})}
function setNotes(value:string){edit(g=>{const b=g.blocks?.find(b=>b.name===selected.value);if(b)b.notes=value})}
function setGraphNotes(value:string){edit(g=>{g.notes=value})}
function chooseInput(node:string,port:string){if(!pending.value)return;const source=pending.value;if(source.node===node)return;edit(g=>{(g.connections??=[]).push({from:`${source.node}.${source.port}`,to:`${node}.${port}`})});pending.value=null;selected.value=null}
function connectBoundary(){edit(g=>{(g.connections??=[]).push({from:boundarySource.value,to:boundaryTarget.value})});boundarySource.value='';boundaryTarget.value=''}
function removeEdge(index:number){edit(g=>{g.connections?.splice(index,1)})}
interface BrickView {key:string;direction:'input'|'output';port:Connector;x:number;y:number}
interface VisibleEdge {path:string;points:GraphPoint[];source:string;unit:string;x:number;y:number;terminal?:{x:number;y:number;direction:string;title:string}}
function portIndex(name:string,port:string,direction:'input'|'output'){
  const block=graph.value.blocks?.find(item=>item.name===name)
  const row=(direction==='input'?block?.inputs:block?.outputs)?.findIndex(item=>item.name===port)??0
  return Math.max(0,row)
}
function portY(name:string,port:string,direction:'input'|'output'){
  const top=props.project.layout[`${prefix.value}${name}`]?.y??80
  return top+74+portIndex(name,port,direction)*28
}
const boundaryCards=computed<BrickView[]>(()=>{
  if(!brickBoundary.value)return []
  const nodes=new Set((graph.value.blocks??[]).map(block=>block.name))
  const grouped=new Map<string,{direction:'input'|'output';port:Connector;desired:number;x:number}>()
  for(const edge of graph.value.connections??[]){
    const [source,target]=edgeEndpoints(edge)
    const [from,fromPort]=source.split('.'),[to,toPort]=target.split('.')
    if(!nodes.has(from)&&nodes.has(to)){
      const key=`input:${source}`,desired=portY(to,toPort,'input')
      const port=graph.value.inputs?.find(item=>item.name===source)??{name:source,type:'Real' as const}
      const x=path.value.length?40:Math.max(20,displayX(to)-250)
      const old=grouped.get(key)
      grouped.set(key,{direction:'input',port,desired:old?(old.desired+desired)/2:desired,x:old?Math.min(old.x,x):x})
    }
    if(nodes.has(from)&&!nodes.has(to)){
      const key=`output:${target}`,desired=portY(from,fromPort,'output')
      const port=graph.value.outputs?.find(item=>item.name===target)??{name:target,type:'Real' as const}
      const x=displayX(from)+330
      const old=grouped.get(key)
      grouped.set(key,{direction:'output',port,desired:old?(old.desired+desired)/2:desired,x:old?Math.max(old.x,x):x})
    }
  }
  const lanes=new Map<string,number>()
  return [...grouped.entries()].sort((a,b)=>a[1].desired-b[1].desired).map(([key,item])=>{
    const lane=`${item.direction}:${Math.round(item.x/20)}`,last=lanes.get(lane)??50
    const center=Math.max(item.desired,last+28)
    lanes.set(lane,center)
    return {key,direction:item.direction,port:item.port,x:item.x,y:center-11}
  })
})
const signalUnits=computed(()=>resolveSignalUnits(graph.value))
const visibleEdges=computed<VisibleEdge[]>(()=>{
  const nodes=new Map((graph.value.blocks??[]).map(block=>[block.name,block]))
  const cards=new Map(boundaryCards.value.map(card=>[card.key,card]))
  const boxes=new Map<string,GraphBox>((graph.value.blocks??[]).map(block=>{
    const top=props.project.layout[`${prefix.value}${block.name}`]?.y??80
    const rows=Math.max(block.inputs?.length??0,block.outputs?.length??0)
    return [block.name,{x:displayX(block.name),y:top,width:280,height:Math.max(140,68+rows*28+(block.category==='composite'?30:0))}]
  }))
  const obstacles=(exclude:string[],cardKey?:string):GraphBox[]=>[
    ...[...boxes].filter(([name])=>!exclude.includes(name)).map(([,box])=>box),
    ...boundaryCards.value.filter(card=>card.key!==cardKey).map(card=>({x:card.x,y:card.y,width:200,height:22})),
  ]
  const useTrackSpacing=(graph.value.connections?.length??0)<=60
  const tracks:RoutedTrack[]=[]
  const route=(start:GraphPoint,end:GraphPoint,source:string,unit:string,boxes:GraphBox[]=[],targetLane?:number):VisibleEdge=>{
    const points=routeWirePoints(start,end,{obstacles:boxes,targetLane:Math.abs(start.y-end.y)<96?targetLane:undefined,tracks:useTrackSpacing?tracks:undefined,source})
    if(useTrackSpacing)tracks.push({source,points})
    return {path:roundedWire(points),points,source,unit,x:end.x,y:end.y}
  }
  return (graph.value.connections??[]).map((edge):VisibleEdge|null=>{
    const [source,target]=edgeEndpoints(edge)
    const unit=signalUnits.value.get(target)??signalUnits.value.get(source)??'unspecified'
    const [from,fromPort]=source.split('.'),[to,toPort]=target.split('.')
    const nodeA=nodes.get(from),nodeB=nodes.get(to)
    if(!nodeA&&!nodeB)return null
    if(!nodeA&&nodeB){
      const end={x:displayX(to),y:portY(to,toPort,'input')}
      const card=cards.get(`input:${source}`)
      if(card){const start={x:card.x+201,y:card.y+11};return route(start,end,source,unit,obstacles([to],card.key))}
      const terminalX=Math.max(0,end.x-62),start={x:terminalX+28,y:end.y}
      return {...route(start,end,source,unit),terminal:{x:terminalX,y:end.y-9,direction:'IN',title:`Graph input: ${source}`}}
    }
    if(nodeA&&!nodeB){
      const start={x:displayX(from)+280,y:portY(from,fromPort,'output')}
      const card=cards.get(`output:${target}`)
      if(card){const end={x:card.x-1,y:card.y+11};return route(start,end,source,unit,obstacles([from],card.key))}
      const end={x:start.x+34,y:start.y}
      return {...route(start,end,source,unit),terminal:{x:end.x,y:end.y-9,direction:'OUT',title:`Graph output: ${target}`}}
    }
    const start={x:displayX(from)+280,y:portY(from,fromPort,'output')}
    const end={x:displayX(to),y:portY(to,toPort,'input')}
    const targetLane=end.x-24-portIndex(to,toPort,'input')*16
    return route(start,end,source,unit,obstacles([from,to]),targetLane)
  }).filter((edge):edge is VisibleEdge=>edge!==null)
})
const visibleUnits=computed(()=>[...new Set(visibleEdges.value.map(edge=>edge.unit))].sort())
const bridges=computed(()=>wireBridges(visibleEdges.value).map(bridge=>{
  const horizontal=visibleEdges.value.find(edge=>edge.points.some((point,index)=>{
    const next=edge.points[index+1]
    return next&&Math.abs(point.y-next.y)<.5&&Math.abs(point.y-bridge.y)<.5&&bridge.x>Math.min(point.x,next.x)&&bridge.x<Math.max(point.x,next.x)
  }))
  return {...bridge,color:signalColor(horizontal?.unit)}
}))
const hasBoundaryEdges=computed(()=>(graph.value.connections??[]).some(edge=>{const [from,to]=edgeEndpoints(edge);return !from.includes('.')||!to.includes('.')}))
function startDrag(event:PointerEvent,name:string){if(event.button!==0)return;const startX=event.clientX,startY=event.clientY;const original=props.project.layout[`${prefix.value}${name}`]??{x:60,y:80};const target=event.currentTarget as HTMLElement;target.setPointerCapture(event.pointerId);function move(e:PointerEvent){edit((_,p)=>{p.layout[`${prefix.value}${name}`]={x:Math.max(0,original.x+(e.clientX-startX)/(zoom.value*columnScale.value)),y:Math.max(0,original.y+(e.clientY-startY)/zoom.value)}})}function stop(){target.removeEventListener('pointermove',move);target.removeEventListener('pointerup',stop);target.removeEventListener('pointercancel',stop)}target.addEventListener('pointermove',move);target.addEventListener('pointerup',stop);target.addEventListener('pointercancel',stop)}
async function setZoom(value:number){const canvas=canvasScroll.value;const previous=zoom.value;zoom.value=Math.min(2,Math.max(.5,value));if(!canvas)return;const ratio=zoom.value/previous;const x=canvas.scrollLeft,y=canvas.scrollTop;await nextTick();canvas.scrollLeft=x*ratio;canvas.scrollTop=y*ratio}
function updateOverflow(){const canvas=canvasScroll.value;if(!canvas)return;overflow.value={left:canvas.scrollLeft>2,right:canvas.scrollLeft+canvas.clientWidth<canvas.scrollWidth-2,top:canvas.scrollTop>2,bottom:canvas.scrollTop+canvas.clientHeight<canvas.scrollHeight-2}}
function panBy(x:number,y:number){const canvas=canvasScroll.value;if(!canvas)return;canvas.scrollBy({left:x*Math.max(160,canvas.clientWidth*.7),top:y*Math.max(160,canvas.clientHeight*.7),behavior:'smooth'})}
function startPan(event:PointerEvent){
  if(event.button!==0||event.target!==event.currentTarget||!canvasScroll.value)return
  const canvas=canvasScroll.value,target=event.currentTarget as HTMLElement
  let x=event.clientX,y=event.clientY
  panning.value=true;activeTooltip.value=null;target.setPointerCapture(event.pointerId)
  event.preventDefault()
  function move(e:PointerEvent){canvas.scrollLeft-=e.clientX-x;canvas.scrollTop-=e.clientY-y;x=e.clientX;y=e.clientY}
  function stop(e:PointerEvent){panning.value=false;target.removeEventListener('pointermove',move);target.removeEventListener('pointerup',stop);target.removeEventListener('pointercancel',stop);if(target.hasPointerCapture(e.pointerId))target.releasePointerCapture(e.pointerId);updateOverflow()}
  target.addEventListener('pointermove',move);target.addEventListener('pointerup',stop);target.addEventListener('pointercancel',stop)
}
function onKeyDown(event:KeyboardEvent){if(event.key==='Escape'&&path.value.length){path.value=[];pending.value=null}}
watch(()=>path.value.join('.'),async()=>{pending.value=null;activeTooltip.value=null;boundarySource.value='';boundaryTarget.value='';await nextTick();canvasScroll.value?.scrollTo(0,0);updateOverflow()})
watch([canvasWidth,canvasHeight,zoom],async()=>{await nextTick();updateOverflow()})
function onResize(){isMobile.value=window.innerWidth<=820}
onMounted(()=>{window.addEventListener('keydown',onKeyDown);window.addEventListener('resize',onResize);canvasObserver=new ResizeObserver(updateOverflow);if(canvasScroll.value)canvasObserver.observe(canvasScroll.value);nextTick(updateOverflow)})
onUnmounted(()=>{window.removeEventListener('keydown',onKeyDown);window.removeEventListener('resize',onResize);canvasObserver?.disconnect()})
</script>
