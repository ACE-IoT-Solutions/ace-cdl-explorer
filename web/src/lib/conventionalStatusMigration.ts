import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const faultEquipment=['heat_pump','boiler','chiller']
const outputs = ['preconditioning_mode', ...Array.from({length:10},(_,index)=>
  `zone_${String(index+1).padStart(2,'0')}_occupancy_confirmed`),
  ...faultEquipment.map(name=>`plant_${name}_start_fault`)]

export function needsConventionalStatusMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeBuildingControls')return false
  return outputs.some(name=>!saved.graph.outputs?.some(port=>port.name===name)
    || !saved.graph.connections?.some(edge=>edgeEndpoints(edge)[1]===name))
}

/** Add explicit CDL status outputs without replacing tuned conventional controls. */
export function migrateConventionalStatus(saved:Project,reference:Block):Project {
  if(!needsConventionalStatusMigration(saved))return saved
  const next=structuredClone(saved)
  next.graph.inputs??=[]
  next.graph.outputs??=[]
  next.graph.blocks??=[]
  next.graph.connections??=[]
  for(const name of ['conventional_preconditioning_off','conventional_occupancy_false',...Array.from({length:10},(_,index)=>
    `zone_${String(index+1).padStart(2,'0')}_occupancy_gate`)]){
    if(next.graph.blocks.some(block=>block.name===name))continue
    const block=reference.blocks?.find(block=>block.name===name)
    if(!block)throw new Error(`Conventional reference lacks ${name}`)
    next.graph.blocks.push(structuredClone(block))
  }
  for(const name of outputs){
    if(!next.graph.outputs.some(port=>port.name===name)){
      const port=reference.outputs?.find(port=>port.name===name)
      if(!port)throw new Error(`Conventional reference lacks ${name}`)
      next.graph.outputs.push(structuredClone(port))
    }
  }
  for(let index=1;index<=10;index++){
    const name=`zone_${String(index).padStart(2,'0')}_occupied`
    if(next.graph.inputs.some(port=>port.name===name))continue
    const port=reference.inputs?.find(port=>port.name===name)
    if(!port)throw new Error(`Conventional reference lacks ${name}`)
    next.graph.inputs.push(structuredClone(port))
  }
  const plant=next.graph.blocks.find(block=>block.name==='plant')
  const plantReference=reference.blocks?.find(block=>block.name==='plant')
  if(!plant||!plantReference)throw new Error('Conventional reference lacks plant dispatch')
  plant.blocks??=[]
  plant.outputs??=[]
  plant.connections??=[]
  for(const equipment of faultEquipment){
    const name=`${equipment}_start_fault`
    const blockName=`conventional_${equipment}_start_fault_off`
    if(!plant.blocks.some(block=>block.name===blockName)){
      const block=plantReference.blocks?.find(block=>block.name===blockName)
      if(!block)throw new Error(`Conventional reference lacks ${blockName}`)
      plant.blocks.push(structuredClone(block))
    }
    if(!plant.outputs.some(port=>port.name===name)){
      const port=plantReference.outputs?.find(port=>port.name===name)
      if(!port)throw new Error(`Conventional reference lacks ${name}`)
      plant.outputs.push(structuredClone(port))
    }
    if(!plant.connections.some(edge=>edgeEndpoints(edge)[1]===name))
      plant.connections.push({from:`${blockName}.y`,to:name})
  }
  const present=new Set(next.graph.connections.map(edge=>edgeEndpoints(edge)[1]))
  for(const edge of reference.connections??[]){
    const [from,to]=edgeEndpoints(edge)
    if(!to.includes('occupancy_gate')&&!outputs.includes(to))continue
    if(!present.has(to)){next.graph.connections.push({from,to});present.add(to)}
  }
  next.layout={...layoutFor(next.graph),...saved.layout}
  return next
}
