import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const modeBlocks = [
  'heat_pump_mode_threshold', 'heat_pump_heating_requested', 'heat_pump_cooling_requested',
  'heat_pump_heating_mode_code', 'heat_pump_cooling_mode_code',
  'heat_pump_mode_cooling_weight', 'heat_pump_cooling_mode_weight', 'heat_pump_mode_sum',
]

export function needsConventionalModeMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeBuildingControls')return false
  const plant=saved.graph.blocks?.find(block=>block.name==='plant')
  return !plant || modeBlocks.some(name=>!plant.blocks?.some(block=>block.name===name))
    || !plant.outputs?.some(port=>port.name==='heat_pump_requested_mode')
    || !saved.graph.outputs?.some(port=>port.name==='plant_heat_pump_requested_mode')
    || !saved.graph.connections?.some(edge=>edgeEndpoints(edge)[1]==='plant_heat_pump_requested_mode')
}

/** Add graph-owned mode selection without replacing any saved plant tuning. */
export function migrateConventionalMode(saved:Project,reference:Block):Project {
  if(!needsConventionalModeMigration(saved))return saved
  const next=structuredClone(saved)
  const plant=next.graph.blocks?.find(block=>block.name==='plant')
  const source=reference.blocks?.find(block=>block.name==='plant')
  if(!plant||!source)throw new Error('Conventional reference lacks plant dispatch')
  plant.blocks??=[]
  plant.outputs??=[]
  plant.connections??=[]
  next.graph.outputs??=[]
  next.graph.connections??=[]
  for(const name of modeBlocks){
    if(plant.blocks.some(block=>block.name===name))continue
    const block=source.blocks?.find(block=>block.name===name)
    if(!block)throw new Error(`Conventional reference lacks ${name}`)
    plant.blocks.push(structuredClone(block))
  }
  if(!plant.outputs.some(port=>port.name==='heat_pump_requested_mode')){
    const port=source.outputs?.find(port=>port.name==='heat_pump_requested_mode')
    if(!port)throw new Error('Conventional reference lacks heat-pump mode port')
    plant.outputs.push(structuredClone(port))
  }
  if(!next.graph.outputs.some(port=>port.name==='plant_heat_pump_requested_mode')){
    const port=reference.outputs?.find(port=>port.name==='plant_heat_pump_requested_mode')
    if(!port)throw new Error('Conventional reference lacks plant heat-pump mode port')
    next.graph.outputs.push(structuredClone(port))
  }
  const plantTargets=new Set((plant.connections??[]).map(edge=>edgeEndpoints(edge)[1]))
  for(const edge of source.connections??[]){
    const [from,to]=edgeEndpoints(edge)
    if(!modeBlocks.some(name=>to.startsWith(`${name}.`))&&to!=='heat_pump_requested_mode')continue
    if(!plantTargets.has(to)){plant.connections.push({from,to});plantTargets.add(to)}
  }
  if(!next.graph.connections.some(edge=>edgeEndpoints(edge)[1]==='plant_heat_pump_requested_mode'))
    next.graph.connections.push({from:'plant.heat_pump_requested_mode',to:'plant_heat_pump_requested_mode'})
  next.layout={...layoutFor(next.graph),...saved.layout}
  return next
}
