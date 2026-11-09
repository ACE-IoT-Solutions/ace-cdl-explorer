import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Replace saved teaching reheat requests with the OBC hot-water request tiers. */
export function migrateG36TerminalHotRequestLevels(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls' || saved.graph.blocks?.find(b=>b.name==='plant')?.blocks?.some(b=>b.name==='terminal_hot_plant_request_active')) return saved
  const next=structuredClone(saved), graph=next.graph
  const plant=graph.blocks?.find(b=>b.name==='plant'), sourcePlant=reference.blocks?.find(b=>b.name==='plant')
  if(!plant||!sourcePlant)throw new Error('G36 hot-water request migration requires the plant composite')
  const endpoints=(e:NonNullable<Block['connections']>[number])=>edgeEndpoints(e)
  const addPort=(target:Block,source:Block,direction:'inputs'|'outputs',name:string)=>{
    const port=source[direction]?.find(p=>p.name===name)
    if(!port)throw new Error(`Current G36 example lacks ${name}`)
    if(!target[direction]?.some(p=>p.name===name))(target[direction]??=[]).push(structuredClone(port))
  }
  const addEdge=(target:Block,from:string,to:string)=>{
    if(!(target.connections??[]).some(e=>{const [a,b]=endpoints(e);return a===from&&b===to}))(target.connections??=[]).push({from,to})
  }
  const zoneBlock=(name:string)=>name.startsWith('hot_reset_')||name.startsWith('hot_plant_')
  for(let i=1;i<=10;i++){
    const name=`zone_${String(i).padStart(2,'0')}`
    const zone=graph.blocks?.find(b=>b.name===name),source=reference.blocks?.find(b=>b.name===name)
    if(!zone||!source)throw new Error(`G36 hot-water request migration requires ${name}`)
    for(const child of source.blocks??[])if(zoneBlock(child.name)&&!zone.blocks?.some(b=>b.name===child.name))(zone.blocks??=[]).push(structuredClone(child))
    for(const suffix of ['hot_water_reset_request','hot_water_plant_request']){
      addPort(zone,source,'outputs',suffix)
      addPort(graph,reference,'outputs',`${name}_${suffix}`)
    }
    for(const e of source.connections??[]){const [from,to]=endpoints(e);if(zoneBlock(from.split('.')[0])||zoneBlock(to.split('.')[0])||to==='hot_water_reset_request'||to==='hot_water_plant_request')addEdge(zone,from,to)}
  }
  const legacyInput=(name:string)=>/^zone_\d\d_(reheat_valve|heating_demand)$/.test(name)
  const oldBlock=(name:string)=>name==='terminal_heat_demand_threshold'||name==='hot_reset_all_request_count'||name.startsWith('terminal_hot_sum_')||/^zone_\d\d_hot_/.test(name)
  plant.inputs=plant.inputs?.filter(p=>!legacyInput(p.name))
  plant.blocks=plant.blocks?.filter(b=>!oldBlock(b.name))
  plant.connections=plant.connections?.filter(e=>{const [from,to]=endpoints(e);return !oldBlock(from.split('.')[0])&&!oldBlock(to.split('.')[0])&&!(to==='hot_reset_request_real.u'&&from!=='hot_reset_with_freeze_requests.y')&&!(from==='hot_water_pump_needed.y'&&to==='freeze_hot_water_pump_enable.u1')})
  for(let i=1;i<=10;i++)for(const suffix of ['hot_water_reset_request','hot_water_plant_request'])addPort(plant,sourcePlant,'inputs',`zone_${String(i).padStart(2,'0')}_${suffix}`)
  for(const name of ['terminal_hot_water_request_count','terminal_hot_water_plant_request_count'])addPort(plant,sourcePlant,'outputs',name)
  for(const name of ['plant_terminal_hot_water_request_count','plant_terminal_hot_water_plant_request_count'])addPort(graph,reference,'outputs',name)
  const newBlock=(name:string)=>name.startsWith('terminal_hot_reset_sum_')||name.startsWith('terminal_hot_plant_sum_')||['hot_reset_all_request_count','terminal_hot_plant_request_active','hot_plant_request_or_heat'].includes(name)
  for(const child of sourcePlant.blocks??[])if(newBlock(child.name)&&!plant.blocks?.some(b=>b.name===child.name))(plant.blocks??=[]).push(structuredClone(child))
  for(const e of sourcePlant.connections??[]){const [from,to]=endpoints(e);if(newBlock(from.split('.')[0])||newBlock(to.split('.')[0])||to==='terminal_hot_water_request_count'||to==='terminal_hot_water_plant_request_count')addEdge(plant,from,to)}
  graph.connections=graph.connections?.filter(e=>!legacyInput(endpoints(e)[1].replace(/^plant\./,'')))
  for(const e of reference.connections??[]){const [from,to]=endpoints(e);if(/^zone_\d\d\.hot_water_(reset|plant)_request$/.test(from)||/^plant\.terminal_hot_water_(plant_)?request_count$/.test(from))addEdge(graph,from,to)}
  next.layout={...layoutFor(graph),...saved.layout}
  return next
}
