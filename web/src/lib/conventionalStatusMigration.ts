import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const outputs = ['preconditioning_mode', ...Array.from({length:10},(_,index)=>
  `zone_${String(index+1).padStart(2,'0')}_occupancy_confirmed`)]

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
  const present=new Set(next.graph.connections.map(edge=>edgeEndpoints(edge)[1]))
  for(const edge of reference.connections??[]){
    const [from,to]=edgeEndpoints(edge)
    if(!to.startsWith('zone_')&&!['preconditioning_mode'].includes(to))continue
    if(!to.includes('occupancy_gate')&&!outputs.includes(to))continue
    if(!present.has(to)){next.graph.connections.push({from,to});present.add(to)}
  }
  next.layout={...layoutFor(next.graph),...saved.layout}
  return next
}
