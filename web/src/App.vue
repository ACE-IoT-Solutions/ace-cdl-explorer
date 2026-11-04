<template>
  <div class="app-shell">
    <header class="site-header"><div class="brand" @click="navigate('primer')"><span class="brand-mark">ACE<span>◆</span></span><span class="brand-divider"/><span class="brand-name">CDL <b>EXPLORER</b></span></div><nav class="top-nav" aria-label="Main navigation"><button :class="{active:tab==='primer'}" @click="navigate('primer')">Primer</button><button :class="{active:tab==='editor'}" @click="navigate('editor')">Visual editor</button><button :class="{active:tab==='simulation'}" @click="navigate('simulation')">Simulation</button><button :class="{active:tab==='glossary'}" @click="navigate('glossary')">Glossary</button></nav><span class="header-tag">BROWSER-BASED CONTROLS LAB</span></header>

    <main v-if="tab==='primer'"><PrimerPage/><section class="cta-band"><div><span class="eyebrow">READY TO EXPERIMENT?</span><h2>Turn the sequence into a working model.</h2></div><button class="solid-button" @click="navigate('editor')">Open the visual editor ↗</button></section></main>

    <main v-else-if="tab==='glossary'"><GlossaryPage/></main>

    <main v-else-if="tab==='editor'" class="workspace-page"><div class="workspace-heading"><div><span class="eyebrow">WORKSPACE / CONTROL GRAPH</span><h1>{{ project?.name ?? 'Loading controls…' }}</h1><p>Build a sequence from blocks, tune parameters, and test its outputs.</p></div><div class="workspace-actions"><select v-model="exampleChoice" aria-label="Load example"><option value="">Load an example…</option><option v-for="example in examples" :key="example.file" :value="example.file">{{ example.label }}</option></select><button class="outline-button" @click="loadExample">Load</button><button class="outline-button" @click="triggerImport">Import JSON</button><button class="outline-button" :disabled="!project" @click="exportProject">Export project</button><input ref="fileInput" class="sr-only" type="file" accept="application/json,.json" @change="importFile"/></div></div>
      <div v-if="project" class="workspace-body"><div class="validation-strip" :class="{invalid:errors.length}"><span>{{ errors.length ? `● ${errors.length} graph issue${errors.length===1?'':'s'}` : '● Graph ready' }}</span><span>{{ project.graph.blocks?.length ?? 0 }} top-level blocks · Local autosave</span></div><div v-if="errors.length" class="validation-list"><p v-for="(error,i) in errors.slice(0,8)" :key="i">{{ error }}</p></div><GraphEditor :key="project.graph.name" :project="project" @change="updateProject"/><section v-if="!isBuildingGraph" class="try-panel"><div><span class="eyebrow">QUICK TEST</span><h2>Run one control step</h2><p>Supply values for the imported sequence and inspect its output.</p></div><div class="try-inputs"><label v-for="input in project.graph.inputs ?? []" :key="input.name" class="field"><span>{{ input.name }} <small>{{ input.unit }}</small></span><select v-if="input.type==='Boolean'" v-model="testInputs[input.name]"><option :value="true">True</option><option :value="false">False</option></select><input v-else type="number" step="any" v-model.number="testInputs[input.name]"/></label></div><button class="solid-button" :disabled="errors.length>0" @click="runStep">Run step</button><pre v-if="testResult">{{ testResult }}</pre></section></div>
    </main>

    <main v-else class="simulation-page"><section class="simulation-hero"><div><span class="eyebrow">THE FOUR-PIPE BUILDING</span><h1>One year. Ten zones. Your control logic.</h1><p>A documented synthetic mixed-climate year drives two AHUs, ten VAVs, and shared hot and chilled water loops. Edit the controllers, then rerun the building to see the response.</p><div class="hero-actions"><AceButton :disabled="!project || !isBuildingGraph || errors.length>0 || running" @click="runYear">{{ running ? 'Running…' : 'Run 2025 simulation' }}</AceButton><button v-if="running" class="outline-button" @click="cancelRun">Cancel</button><button class="outline-button" @click="navigate('editor')">Edit controls ↗</button></div><p v-if="!isBuildingGraph" class="simulation-notice">Load the Four-pipe building example in the editor to run the building simulation.</p></div><div class="hero-stat"><span>2025 SAMPLE YEAR</span><strong>35,040</strong><small>15-minute steps · 2 AHUs · 10 VAV zones</small></div></section>
      <div class="sim-layout"><aside class="sim-side"><div class="panel-title"><span class="eyebrow">SCENARIO</span><h2>Building assumptions</h2></div><p class="muted small">These simplified thermal assumptions apply to all ten zones.</p><label class="field"><span>Zone envelope · kW/K</span><input v-model.number="scenario.zone_ua_kw_per_k" type="number" min="0.01" max="2" step="0.01" @change="saveScenario"/></label><label class="field"><span>Thermal capacity · kWh/K</span><input v-model.number="scenario.zone_capacity_kwh_per_k" type="number" min="0.1" max="50" step="0.1" @change="saveScenario"/></label><label class="field"><span>Heat pump share · %</span><input :value="Math.round(scenario.heat_pump_fraction*100)" type="range" min="0" max="100" step="5" @input="scenario.heat_pump_fraction=Number(($event.target as HTMLInputElement).value)/100;saveScenario()"/><b>{{ Math.round(scenario.heat_pump_fraction*100) }}%</b></label><div class="assumption-note">Each AHU mixes 20% outdoor air with return air. Fan speed limits delivered airflow; plant output limits the supply-air coil and reheat. Energy counts heat and cooling actually used.</div></aside>
      <div class="sim-content"><div class="sim-status"><div><span class="status-dot">● {{ running ? 'Running in your browser' : results.length ? 'Custom run complete' : 'Bundled baseline' }}</span><strong>{{ running ? `${Math.round(progress/35040*100)}% complete` : '2025 · January → December' }}</strong></div><div v-if="running" class="progress-track"><i :style="{width:`${progress/35040*100}%`} "/></div></div><div v-if="simError" class="error-banner">{{ simError }}</div>
      <div class="chart-card signal-card">
        <div class="card-heading"><div><span class="eyebrow">CONTROL SIGNAL EXPLORER</span><h2>Compare process variables</h2></div><span class="muted small">{{ signalOptions.length }} selectable signals</span></div>
        <div class="signal-picker"><label class="field"><span>Find a variable or connection</span><input v-model="signalSearch" type="search" placeholder="Search block, port, Brick class, or unit…" @input="candidateSignal=''"/></label><label class="field"><span>Signal</span><select v-model="candidateSignal" :disabled="running || !isBuildingGraph"><option value="">Choose a signal…</option><option v-for="signal in filteredSignals" :key="signal.id" :value="signal.id">{{ signal.kind==='physical' ? 'Physical · ' : signal.kind==='connection' ? 'Connection · ' : signal.kind==='input' ? 'Input · ' : 'Output · ' }}{{ signal.label }} ({{ signal.unit }}){{ signal.brickClass ? ` · brick:${signal.brickClass}` : '' }}</option></select></label><button class="outline-button signal-add" type="button" :disabled="!candidateSignal || running" @click="addSignal">Add signal</button></div>
        <div v-if="selectedSignalDetails.length" class="signal-selections"><div v-for="signal in selectedSignalDetails" :key="signal.id" class="signal-chip"><span><b>{{ signal.unit }}</b>{{ signal.label }}<small v-if="signal.brickClass"> · brick:{{ signal.brickClass }}</small></span><button type="button" :aria-label="`Remove ${signal.label}`" :disabled="running" @click="removeSignal(signal.id)">×</button></div></div>
        <p v-if="!selectedSignalDetails.length" class="muted small">Add several graph or physical signals. Each unit type gets its own plot with a labeled y-axis.</p>
        <div v-else-if="needsSignalRun" class="signal-run"><span>{{ results.length ? 'New graph signals need a fresh run.' : 'Run the current model to record your selected signals.' }}</span><button class="solid-button" type="button" :disabled="running || !isBuildingGraph || errors.length>0" @click="runYear">{{ running ? 'Running…' : 'Run selected signals' }}</button></div>
        <div class="time-controls"><label class="field"><span>From · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeStart)" @change="setTimeRange(timeIndex(($event.target as HTMLInputElement).value),timeEnd)"/></label><label class="field"><span>To · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeEnd)" @change="setTimeRange(timeStart,timeIndex(($event.target as HTMLInputElement).value))"/></label><div class="time-zoom"><button class="outline-button" type="button" aria-label="Zoom time out" @click="zoomTime(2)">−</button><button class="outline-button" type="button" aria-label="Reset time range" @click="setTimeRange(0,35039)">Full year</button><button class="outline-button" type="button" aria-label="Zoom time in" @click="zoomTime(0.5)">+</button></div></div>
      </div>
      <SignalChart v-for="group in plotGroups" :key="group.unit" :rows="results" :signals="group.signals" :unit="group.unit" :start="timeStart" :end="timeEnd" @range="setTimeRange"/>
      <div class="metric-grid"><article><span>ELECTRICITY</span><strong>{{ number(summary.electricity_kwh) }}</strong><small>kWh · heat pump, chiller, fans</small></article><article><span>BOILER FUEL</span><strong>{{ number(summary.gas_kwh) }}</strong><small>kWh input</small></article><article><span>UNMET COMFORT</span><strong>{{ number(summary.unmet_zone_hours) }}</strong><small>zone-hours</small></article><article><span>AVG ZONE TEMP</span><strong>{{ latestRow?.avg_zone_c.toFixed(1) ?? '—' }}°C</strong><small>latest time step</small></article></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">TEMPERATURE RESPONSE</span><h2>Zone and outdoor temperature</h2></div><span class="muted small">{{ results.length ? 'CUSTOM RUN' : 'BASELINE' }}</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :start="timeStart" :end="timeEnd" field-a="avg_zone_c" field-b="outdoor_c" unit="°C" label-a="Average zone" label-b="Outdoor air"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">AIR DELIVERY</span><h2>VAV request and fan-delivered airflow</h2></div><span class="muted small">NORMALIZED FLOW UNITS</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :start="timeStart" :end="timeEnd" field-a="requested_airflow" field-b="delivered_airflow" unit="normalized airflow" :zero-based="true" label-a="VAV request" label-b="Fan delivered"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">COIL RESPONSE</span><h2>AHU 1 supply air</h2></div><span class="muted small">TARGET / ACTUAL</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :start="timeStart" :end="timeEnd" field-a="ahu_1_supply_setpoint_c" field-b="ahu_1_supply_actual_c" unit="°C" label-a="Target · °C" label-b="Actual · °C"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">ENERGY PROFILE</span><h2>Electricity and boiler fuel</h2></div><button v-if="results.length" class="text-button" @click="exportResults">Download CSV ↓</button></div><TrendChart v-if="displayRows.length" :rows="displayRows" :start="timeStart" :end="timeEnd" field-a="electric_kw" field-b="gas_kw" unit="kW" :zero-based="true" label-a="Electric · kW" label-b="Gas · kW"/></div><div class="zone-card"><div class="card-heading"><div><span class="eyebrow">ZONE SNAPSHOT</span><h2>Latest temperatures</h2></div><span class="muted small">AHU 1 · Z1–5 / AHU 2 · Z6–10</span></div><div class="zone-grid"><div v-for="(temp,i) in latestRow?.zone_c ?? []" :key="i"><span>ZONE {{ String(i+1).padStart(2,'0') }}</span><strong>{{ temp.toFixed(1) }}°C</strong></div></div></div></div></div>
    </main>
    <footer class="site-footer"><span>ACE IoT · CDL Explorer</span><span>Simplified educational simulation · Runs locally in your browser</span><a href="https://github.com/ACE-IoT-Solutions/python-cdl" target="_blank" rel="noreferrer">Python CDL reference ↗</a></footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, shallowRef } from 'vue'
import PrimerPage from './components/PrimerPage.vue'
import GlossaryPage from './components/GlossaryPage.vue'
import GraphEditor from './components/GraphEditor.vue'
import TrendChart from './components/TrendChart.vue'
import SignalChart from './components/SignalChart.vue'
import AceButton from './vendor/ace/AceButton.vue'
import { defaultScenario, edgeEndpoints, layoutFor, type Block, type Project, type Row, type Scenario, type Summary } from './lib/types'
import { graphSignals, physicalSignals, type GraphSignal } from './lib/signals'
import RuntimeWorker from './workers/runtime.worker?worker'

type Tab = 'primer' | 'editor' | 'simulation' | 'glossary'
function routeFromHash():Tab {const route=location.hash.slice(1);return route==='editor'||route==='simulation'||route==='glossary'?route:'primer'}
const tab=ref<Tab>(routeFromHash())
const project=shallowRef<Project|null>(null)
const errors=ref<string[]>([])
const worker=new RuntimeWorker()
const exampleChoice=ref('')
const fileInput=ref<HTMLInputElement|null>(null)
const examples=[{file:'four_pipe_building.json',label:'Four-pipe building · nested controls'},{file:'vav_composite_template.json',label:'VAV composite · editable blocks'},{file:'p_controller_limiter.json',label:'P controller with limiter'},{file:'room_control_system.json',label:'Dual-mode room controller'},{file:'simple_temp_controller.json',label:'Temperature controller'},{file:'basic_temp_control.json',label:'Basic temperature controller'},...['east','north','south','west','corridor'].map(x=>({file:`${x}_controller.json`,label:`Legacy VAV controller · ${x}`}))]
const testInputs=reactive<Record<string,number|boolean>>({})
const testResult=ref('')
const scenario=reactive<Scenario>({...defaultScenario})
const baseline=shallowRef<Row[]>([])
const baselineSummary=ref<Summary>({electricity_kwh:0,gas_kwh:0,unmet_zone_hours:0})
const results=shallowRef<Row[]>([])
const resultSummary=ref<Summary|null>(null)
const running=ref(false),progress=ref(0),simError=ref('')
const signalSearch=ref(''),candidateSignal=ref(''),selectedSignals=ref<string[]>([]),activeTraceIds=ref<string[]>([]),timeStart=ref(0),timeEnd=ref(35039)
let runBuffer:Row[]=[]
let jobId=0
const isBuildingGraph=computed(()=>{const blocks=project.value?.graph.blocks??[];return blocks.filter(b=>b.block_type==='VAVBoxController'&&b.blocks?.length).length===10&&blocks.filter(b=>b.block_type==='AHUController').length===2&&blocks.some(b=>b.block_type==='PlantDispatch')})
const displayRows=computed(()=>results.value.length?results.value:baseline.value)
const summary=computed(()=>resultSummary.value??baselineSummary.value)
const latestRow=computed(()=>displayRows.value.at(-1))
const signalOptions=computed(()=>project.value?[...physicalSignals,...graphSignals(project.value.graph)]:[])
const filteredSignals=computed(()=>{const query=signalSearch.value.trim().toLowerCase();return signalOptions.value.filter(signal=>!selectedSignals.value.includes(signal.id)&&(!query||`${signal.label} ${signal.unit} ${signal.brickClass??''}`.toLowerCase().includes(query)))})
const selectedSignalDetails=computed(()=>selectedSignals.value.flatMap(id=>{const signal=signalOptions.value.find(item=>item.id===id);return signal?[signal]:[]}))
const needsSignalRun=computed(()=>selectedSignals.value.length>0&&(!results.value.length||selectedSignalDetails.value.some(signal=>signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))))
const plotGroups=computed(()=>{if(!results.value.length)return [];const groups=new Map<string,GraphSignal[]>();for(const signal of selectedSignalDetails.value){if(signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))continue;const series=groups.get(signal.unit)??[];series.push(signal);groups.set(signal.unit,series)}return [...groups].map(([unit,signals])=>({unit,signals}))})
const number=(n:number)=>Math.round(n).toLocaleString()
const asset=(name:string)=>new URL(`examples/${name}`,document.baseURI).toString()
async function fetchJson<T>(name:string):Promise<T>{const response=await fetch(asset(name));if(!response.ok)throw new Error(`Could not load ${name}`);return response.json() as Promise<T>}
function syncInputs(graph:Block){for(const key of Object.keys(testInputs))delete testInputs[key];for(const input of graph.inputs??[])testInputs[input.name]=input.type==='Boolean'?false:input.name.toLowerCase().includes('max')?20:input.name.toLowerCase().includes('temp')?22:1;testResult.value=''}
function updateProject(next:Project){project.value=next;results.value=[];resultSummary.value=null;activeTraceIds.value=[];localStorage.setItem('ace-cdl-project-v1',JSON.stringify(next));validateGraph(next.graph)}
async function fillExampleMetadata(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeBuildingControls')return saved
  try{
    const reference=await fetchJson<Block>('four_pipe_building.json')
    const next=structuredClone(saved)
    let changed=false
    function visit(block:Block,source:Block){
      if(block.notes===undefined&&source.notes){block.notes=source.notes;changed=true}
      for(const direction of ['inputs','outputs'] as const){
        for(const port of block[direction]??[]){
          const reference=source[direction]?.find(item=>item.name===port.name)
          if(!reference)continue
          for(const key of ['label','unit','brick_class','description'] as const){
            if(!port[key]&&reference[key]){port[key]=reference[key];changed=true}
          }
        }
      }
      for(const child of block.blocks??[]){const match=source.blocks?.find(item=>item.name===child.name&&item.block_type===child.block_type);if(match)visit(child,match)}
    }
    visit(next.graph,reference)
    return changed?next:saved
  }catch{return saved}
}
function upgradeSupplyFeedback(saved:Project):Project{
  if(saved.graph.name!=='FourPipeBuildingControls')return saved
  const next=structuredClone(saved)
  let changed=false
  for(let ahu=1;ahu<=2;ahu++){
    let rewired=false
    const measured=`ahu_${ahu}_actual_supply_air_temp`
    for(let zone=(ahu-1)*5+1;zone<=ahu*5;zone++){
      const target=`zone_${String(zone).padStart(2,'0')}.supply_air_temp`
      const oldSource=`ahu_${ahu}.supply_air_temp`
      const edge=next.graph.connections?.find(item=>{const [from,to]=edgeEndpoints(item);return from===oldSource&&to===target})
      if(edge){edge.from=measured;delete edge.from_block;delete edge.from_output;changed=true;rewired=true}
    }
    if(rewired&&!next.graph.inputs?.some(port=>port.name===measured)){
      ;(next.graph.inputs??=[]).push({name:measured,type:'Real',unit:'°C',description:`Measured AHU ${ahu} supply air temperature from the previous step`})
    }
  }
  return changed?next:saved
}
function upgradeLayout(saved:Project):Project{
  const previous=layoutFor(saved.graph,true),improved=layoutFor(saved.graph)
  const next=structuredClone(saved)
  let changed=false
  for(const [name,oldPosition] of Object.entries(previous)){
    const position=next.layout[name],fresh=improved[name]
    if(position&&fresh&&position.x===oldPosition.x&&position.y===oldPosition.y&&(position.x!==fresh.x||position.y!==fresh.y)){next.layout[name]=fresh;changed=true}
  }
  return changed?next:saved
}
function validateGraph(graph:Block){worker.postMessage({id:++jobId,type:'validate',graph})}
async function loadExample(){if(!exampleChoice.value)return;try{const graph=await fetchJson<Block>(exampleChoice.value);const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);Object.assign(scenario,next.scenario);syncInputs(graph);results.value=[];resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}}
function triggerImport(){fileInput.value?.click()}
async function importFile(event:Event){const file=(event.target as HTMLInputElement).files?.[0];if(!file)return;try{const data=JSON.parse(await file.text());const graph:Block=data.version===1?data.graph:data;if(!graph?.name||!graph.block_type)throw new Error('Expected a CDL block or ACE CDL project');const next:Project=data.version===1?{...data,layout:data.layout??layoutFor(graph),scenario:data.scenario??{...defaultScenario}}:{version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);Object.assign(scenario,next.scenario);syncInputs(graph);results.value=[];resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}finally{(event.target as HTMLInputElement).value=''}}
function download(name:string,contents:string,type='application/json'){const url=URL.createObjectURL(new Blob([contents],{type}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000)}
function exportProject(){if(project.value)download(`${project.value.name}.ace-cdl.json`,JSON.stringify(project.value,null,2))}
function exportResults(){
  const zoneColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_c`)
  const header=['index','outdoor_c','avg_zone_c',...zoneColumns,'requested_airflow','delivered_airflow','ahu_1_supply_setpoint_c','ahu_1_supply_actual_c','ahu_2_supply_setpoint_c','ahu_2_supply_actual_c','cool_request_kw','cool_kw','heat_request_kw','heat_kw','heat_pump_heating_used_kw','heat_pump_cooling_used_kw','boiler_used_kw','chiller_used_kw','electric_kw','gas_kw']
  const rows=results.value.map(r=>[r.index,r.outdoor_c,r.avg_zone_c,...r.zone_c,r.requested_airflow,r.delivered_airflow,r.ahu_1_supply_setpoint_c,r.ahu_1_supply_actual_c,r.ahu_2_supply_setpoint_c,r.ahu_2_supply_actual_c,r.cool_request_kw,r.cool_kw,r.heat_request_kw,r.heat_kw,r.heat_pump_heating_used_kw,r.heat_pump_cooling_used_kw,r.boiler_used_kw,r.chiller_used_kw,r.electric_kw,r.gas_kw].join(','))
  download('ace-cdl-simulation-2025.csv',[header.join(','),...rows].join('\n'),'text/csv')
}
function runStep(){if(project.value)worker.postMessage({id:++jobId,type:'execute',graph:project.value.graph,inputs:{...testInputs}})}
function saveScenario(){if(!project.value)return;const next=structuredClone(project.value);next.scenario={...scenario};updateProject(next)}
let baselineLoading:Promise<void>|null=null
function ensureBaseline(){if(baseline.value.length||baselineLoading)return;baselineLoading=fetchJson<{rows:Row[];summary:Summary}>('baseline_2025.json').then(data=>{baseline.value=data.rows;baselineSummary.value=data.summary;simError.value=''}).catch(e=>{simError.value=String(e)}).finally(()=>{baselineLoading=null})}
function syncRoute(){tab.value=routeFromHash();if(tab.value==='simulation')ensureBaseline()}
function navigate(target:Tab){if(location.hash!==`#${target}`)location.hash=target;syncRoute()}
const firstStep=Date.UTC(2025,0,1)
function timeValue(index:number){return new Date(firstStep+index*900000).toISOString().slice(0,16)}
function timeIndex(value:string){const stamp=Date.parse(`${value}Z`);return Number.isFinite(stamp)?Math.round((stamp-firstStep)/900000):0}
function setTimeRange(start:number,end:number){const low=Math.max(0,Math.min(35035,Math.round(start)));timeStart.value=low;timeEnd.value=Math.max(low+4,Math.min(35039,Math.round(end)))}
function zoomTime(scale:number){const center=(timeStart.value+timeEnd.value)/2;const width=Math.max(4,Math.min(35039,Math.round((timeEnd.value-timeStart.value)*scale)));let start=Math.round(center-width/2);start=Math.max(0,Math.min(35039-width,start));setTimeRange(start,start+width)}
function addSignal(){if(!candidateSignal.value||selectedSignals.value.includes(candidateSignal.value))return;selectedSignals.value=[...selectedSignals.value,candidateSignal.value];candidateSignal.value=''}
function removeSignal(id:string){selectedSignals.value=selectedSignals.value.filter(item=>item!==id)}
async function runYear(){if(!project.value||!isBuildingGraph.value||running.value)return;running.value=true;simError.value='';results.value=[];runBuffer=[];resultSummary.value=null;progress.value=0;const graph=project.value.graph;const traces=selectedSignalDetails.value.filter(signal=>signal.kind!=='physical').map(signal=>signal.id);activeTraceIds.value=traces;try{const weather=await fetchJson<unknown[]>('weather_2025.json');worker.postMessage({id:++jobId,type:'simulate',graph,scenario:{...scenario},weather,traces})}catch(e){simError.value=String(e);running.value=false}}
function cancelRun(){worker.postMessage({type:'cancel'});running.value=false}
worker.onmessage=({data})=>{if(data.type==='validation'){errors.value=data.errors}else if(data.type==='result'){testResult.value=JSON.stringify(data.result,null,2)}else if(data.type==='batch'){runBuffer.push(...data.rows);progress.value=data.progress;resultSummary.value=data.summary;if(data.done||data.progress%1536===0)results.value=[...runBuffer];if(data.done)running.value=false}else if(data.type==='cancelled'){running.value=false}else if(data.type==='error'){simError.value=data.message;testResult.value=data.message;running.value=false}}
onMounted(async()=>{window.addEventListener('hashchange',syncRoute);syncRoute();try{const stored=localStorage.getItem('ace-cdl-project-v1');if(stored){let parsed=JSON.parse(stored) as Project;if(parsed.graph.name==='FourPipeBuildingControls'&&!(parsed.graph.blocks??[]).some(b=>b.block_type==='PlantDispatch')){localStorage.setItem('ace-cdl-legacy-project-backup',stored);const graph=await fetchJson<Block>('four_pipe_building.json');for(const old of parsed.graph.blocks??[]){const next=graph.blocks?.find(b=>b.name===old.name);if(!next)continue;const oldParam=(name:string)=>old.parameters?.find(p=>p.name===name)?.value;for(const [oldName,childName] of [['cooling_setpoint','cool_setpoint'],['heating_setpoint','heat_setpoint'],['min_damper_position','minimum_damper'],['min_airflow','minimum_airflow']] as const){const value=oldParam(oldName);const p=next.blocks?.find(b=>b.name===childName)?.parameters?.find(p=>p.name==='k');if(p&&typeof value==='number')p.value=value}}parsed={version:1,name:graph.name,graph,scenario:parsed.scenario??{...defaultScenario},layout:layoutFor(graph)};updateProject(parsed)}else{const enriched=upgradeLayout(await fillExampleMetadata(upgradeSupplyFeedback(parsed)));if(enriched===parsed){project.value=parsed;validateGraph(parsed.graph)}else{parsed=enriched;updateProject(parsed)}}Object.assign(scenario,parsed.scenario);syncInputs(parsed.graph)}else{const graph=await fetchJson<Block>('four_pipe_building.json');const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);syncInputs(graph)}}catch(e){errors.value=[String(e)]}})
onUnmounted(()=>{window.removeEventListener('hashchange',syncRoute);worker.terminate()})
</script>
