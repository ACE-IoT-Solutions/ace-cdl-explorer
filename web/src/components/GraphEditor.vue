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
        <div class="toolbar-actions"><span v-if="pending" class="connect-hint">Connect {{ pending.node }}.{{ pending.port }} → choose input</span><button v-if="pending" class="text-button" @click="pending=null">Cancel</button><span class="pan-hint">Drag background to pan</span><span>{{ graph.blocks?.length ?? 0 }} blocks</span><div class="zoom-controls" aria-label="Canvas zoom"><button type="button" aria-label="Zoom out" :disabled="zoom<=0.5" @click="setZoom(zoom-0.25)">−</button><button type="button" class="zoom-level" aria-label="Reset zoom to 100 percent" @click="setZoom(1)">{{ Math.round(zoom*100) }}%</button><button type="button" aria-label="Zoom in" :disabled="zoom>=2" @click="setZoom(zoom+0.25)">+</button></div><button v-if="path.length" class="modal-close" aria-label="Close nested editor" @click="path=[]">×</button></div>
      </div>
      <div class="canvas-viewport"><div ref="canvasScroll" class="canvas-scroll" @scroll="updateOverflow">
        <div class="canvas-sizer" :style="{ width: canvasWidth*zoom+'px', height: canvasHeight*zoom+'px' }"><div class="graph-canvas" :class="{ panning }" :style="{ width: canvasWidth+'px', height: canvasHeight+'px', transform:`scale(${zoom})` }" @pointerdown="startPan">
          <svg class="edge-layer" :width="canvasWidth" :height="canvasHeight" aria-hidden="true">
            <g v-for="(edge,index) in visibleEdges" :key="index"><path :d="edge.path" fill="none" stroke="var(--ace-secondary)" stroke-width="2" opacity=".75"/><circle :cx="edge.x" :cy="edge.y" r="3" fill="var(--ace-secondary)"/></g>
          </svg>
          <article v-for="child in graph.blocks ?? []" :key="child.name" class="graph-node" :class="{ selected: selected===child.name, 'has-tooltip':activeTooltip===child.name }" :style="position(child.name)" @click="selected=child.name;activeTooltip=null" @mouseenter="activeTooltip=child.name" @mouseleave="!isMobile && (activeTooltip=null)" @focusin="activeTooltip=child.name" @focusout="hideTooltipOnBlur($event,child.name)">
            <div class="node-head" @pointerdown="startDrag($event,child.name)"><span>{{ child.name }}</span><span class="node-kind">{{ shortType(child.block_type) }}</span><button class="node-info" type="button" :aria-label="`About ${child.name}`" :aria-describedby="activeTooltip===child.name ? tooltipId(child.name) : undefined" @pointerdown.stop @click.stop="activeTooltip=child.name">i</button></div>
            <div class="node-ports">
              <div class="input-ports"><button v-for="port in child.inputs ?? []" :key="port.name" class="port-row" :title="`${port.type} input`" @click.stop="chooseInput(child.name,port.name)"><i class="port-dot input-dot"/>{{ port.name }}</button></div>
              <div class="output-ports"><button v-for="port in child.outputs ?? []" :key="port.name" class="port-row" :title="`${port.type} output`" @click.stop="pending={node:child.name,port:port.name}">{{ port.name }}<i class="port-dot output-dot"/></button></div>
            </div>
            <button v-if="child.category==='composite' || child.blocks?.length" class="node-open" @click.stop="path=[...path,child.name];selected=null">Open composite ↗</button>
            <Teleport to="body" :disabled="!isMobile"><div v-if="activeTooltip===child.name" :id="tooltipId(child.name)" class="node-tooltip" :class="{ 'tooltip-left':(project.layout[`${prefix}${child.name}`]?.x ?? 0)>canvasWidth-420 }" role="tooltip">
              <button v-if="isMobile" class="tooltip-close" type="button" aria-label="Close block information" @click.stop="activeTooltip=null">×</button>
              <strong>{{ child.name }}</strong><span class="tooltip-kind">{{ shortType(child.block_type) }}{{ child.category==='composite' ? ' · composite' : '' }}</span>
              <p>{{ blockDescription(child) }}</p>
              <div v-if="child.notes?.trim()" class="tooltip-notes"><b>EXAMPLE NOTE</b><span>{{ child.notes }}</span></div>
              <div class="tooltip-group"><b>INPUTS · {{ child.inputs?.length ?? 0 }}</b><div v-if="!child.inputs?.length" class="tooltip-empty">No input ports</div><div v-for="port in child.inputs ?? []" :key="`in-${port.name}`" class="tooltip-port"><span>{{ port.name }} <small>{{ port.type }}{{ port.unit ? ` · ${port.unit}` : '' }}</small></span><em>{{ portDescription(child,'input',port) }}</em></div></div>
              <div class="tooltip-group"><b>OUTPUTS · {{ child.outputs?.length ?? 0 }}</b><div v-if="!child.outputs?.length" class="tooltip-empty">No output ports</div><div v-for="port in child.outputs ?? []" :key="`out-${port.name}`" class="tooltip-port"><span>{{ port.name }} <small>{{ port.type }}{{ port.unit ? ` · ${port.unit}` : '' }}</small></span><em>{{ portDescription(child,'output',port) }}</em></div></div>
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
        <label class="field"><span>From</span><select v-model="boundarySource"><option value="">Select output</option><option v-for="port in graph.inputs ?? []" :key="port.name" :value="port.name">Input: {{ port.name }}</option><template v-for="child in graph.blocks ?? []" :key="child.name"><option v-for="port in child.outputs ?? []" :key="port.name" :value="`${child.name}.${port.name}`">{{ child.name }}.{{ port.name }}</option></template></select></label>
        <label class="field"><span>To</span><select v-model="boundaryTarget"><option value="">Select input</option><option v-for="port in graph.outputs ?? []" :key="port.name" :value="port.name">Output: {{ port.name }}</option><template v-for="child in graph.blocks ?? []" :key="child.name"><option v-for="port in child.inputs ?? []" :key="port.name" :value="`${child.name}.${port.name}`">{{ child.name }}.{{ port.name }}</option></template></select></label>
        <button class="solid-button" :disabled="!boundarySource || !boundaryTarget" @click="connectBoundary">Add connection</button>
      </template>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { edgeEndpoints, palette, type Block, type Connector, type Project } from '../lib/types'
import { blockDescription, portDescription } from '../lib/blockHelp'

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
const canvasWidth = computed(() => Math.max(1340,...(graph.value.blocks ?? []).map(b => (props.project.layout[`${prefix.value}${b.name}`]?.x ?? 0)+340)))
const canvasHeight = computed(() => Math.max(660,...(graph.value.blocks ?? []).map(b => (props.project.layout[`${prefix.value}${b.name}`]?.y ?? 0)+350)))
function position(name: string) {const p=props.project.layout[`${prefix.value}${name}`] ?? {x:60,y:80};return {left:`${p.x}px`,top:`${p.y}px`}}
function shortType(type: string) {return type.split('.').at(-1) ?? type}
function tooltipId(name:string){return `help-${[...path.value,name].join('-').replace(/[^A-Za-z0-9_-]/g,'-')}`}
function hideTooltipOnBlur(event:FocusEvent,name:string){const node=event.currentTarget as HTMLElement;if(!node.contains(event.relatedTarget as Node)&&activeTooltip.value===name)activeTooltip.value=null}
function edit(fn:(graph:Block,project:Project)=>void) {const project=structuredClone(props.project);let graph=project.graph;for(const name of path.value){graph=(graph.blocks ?? []).find(b=>b.name===name)!}fn(graph,project);emit('change',project)}
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
function addPort(){const name=newPortName.value.trim();if(!name||!selected.value||!/^[A-Za-z][A-Za-z0-9_]*$/.test(name))return;edit(g=>{const b=g.blocks?.find(b=>b.name===selected.value);if(!b||[...(b.inputs??[]),...(b.outputs??[])].some(p=>p.name===name))return;const key=newPortDirection.value==='input'?'inputs':'outputs';(b[key]??=[]).push({name,type:newPortType.value})});newPortName.value=''}
function removeBlock(){if(!selected.value)return;const name=selected.value;edit(g=>{g.blocks=(g.blocks??[]).filter(b=>b.name!==name);g.connections=(g.connections??[]).filter(e=>{const [a,b]=edgeEndpoints(e);return a.split('.')[0]!==name&&b.split('.')[0]!==name})});selected.value=null}
function renameBlock(name:string){if(!selected.value||!name.trim()||graph.value.blocks?.some(b=>b.name===name&&b.name!==selected.value))return;const old=selected.value;edit((g,p)=>{const b=g.blocks!.find(b=>b.name===old)!;b.name=name;for(const e of g.connections??[]){const [a,z]=edgeEndpoints(e);if(a.startsWith(`${old}.`)){e.from=`${name}.${a.split('.').slice(1).join('.')}`;delete e.from_block;delete e.from_output}if(z.startsWith(`${old}.`)){e.to=`${name}.${z.split('.').slice(1).join('.')}`;delete e.to_block;delete e.to_input}}p.layout[`${prefix.value}${name}`]=p.layout[`${prefix.value}${old}`];delete p.layout[`${prefix.value}${old}`]});selected.value=name}
function setParameter(name:string,value:number|boolean){edit(g=>{const p=g.blocks?.find(b=>b.name===selected.value)?.parameters?.find(p=>p.name===name);if(p)p.value=value})}
function setNotes(value:string){edit(g=>{const b=g.blocks?.find(b=>b.name===selected.value);if(b)b.notes=value})}
function setGraphNotes(value:string){edit(g=>{g.notes=value})}
function chooseInput(node:string,port:string){if(!pending.value)return;const source=pending.value;if(source.node===node)return;edit(g=>{(g.connections??=[]).push({from:`${source.node}.${source.port}`,to:`${node}.${port}`})});pending.value=null;selected.value=null}
function connectBoundary(){edit(g=>{(g.connections??=[]).push({from:boundarySource.value,to:boundaryTarget.value})});boundarySource.value='';boundaryTarget.value=''}
function removeEdge(index:number){edit(g=>{g.connections?.splice(index,1)})}
const visibleEdges=computed(()=>{
  const nodes=new Map((graph.value.blocks??[]).map(b=>[b.name,b]))
  return (graph.value.connections??[]).map(e=>{
    const [a,b]=edgeEndpoints(e)
    const [from,portA]=a.split('.'),[to,portB]=b.split('.')
    const nodeA=nodes.get(from),nodeB=nodes.get(to)
    if(!nodeA&&!nodeB)return null
    if(!nodeA&&nodeB){
      const p=props.project.layout[`${prefix.value}${to}`]??{x:60,y:80}
      const y=p.y+72+Math.max(0,(nodeB.inputs??[]).findIndex(c=>c.name===portB))*24
      return {path:`M${p.x-25} ${y} L${p.x} ${y}`,x:p.x-25,y}
    }
    if(nodeA&&!nodeB){
      const p=props.project.layout[`${prefix.value}${from}`]??{x:60,y:80}
      const y=p.y+72+Math.max(0,(nodeA.outputs??[]).findIndex(c=>c.name===portA))*24
      return {path:`M${p.x+280} ${y} L${p.x+305} ${y}`,x:p.x+305,y}
    }
    const pA=props.project.layout[`${prefix.value}${from}`]??{x:60,y:80}
    const pB=props.project.layout[`${prefix.value}${to}`]??{x:60,y:80}
    const x1=pA.x+280,y1=pA.y+72+Math.max(0,(nodeA!.outputs??[]).findIndex(p=>p.name===portA))*24
    const x2=pB.x,y2=pB.y+72+Math.max(0,(nodeB!.inputs??[]).findIndex(p=>p.name===portB))*24
    const bend=Math.max(32,Math.min(90,(x2-x1)/2))
    return {path:`M${x1} ${y1} C${x1+bend} ${y1},${x2-bend} ${y2},${x2} ${y2}`,x:x2,y:y2}
  }).filter((x):x is {path:string;x:number;y:number}=>x!==null)
})
function startDrag(event:PointerEvent,name:string){if(event.button!==0)return;const startX=event.clientX,startY=event.clientY;const original=props.project.layout[`${prefix.value}${name}`]??{x:60,y:80};const target=event.currentTarget as HTMLElement;target.setPointerCapture(event.pointerId);function move(e:PointerEvent){edit((_,p)=>{p.layout[`${prefix.value}${name}`]={x:Math.max(0,original.x+(e.clientX-startX)/zoom.value),y:Math.max(0,original.y+(e.clientY-startY)/zoom.value)}})}function stop(){target.removeEventListener('pointermove',move);target.removeEventListener('pointerup',stop);target.removeEventListener('pointercancel',stop)}target.addEventListener('pointermove',move);target.addEventListener('pointerup',stop);target.addEventListener('pointercancel',stop)}
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
