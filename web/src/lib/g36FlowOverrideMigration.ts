import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const overrideBlocks = (name:string)=>name.startsWith('flow_override_')

export function needsG36FlowOverrideMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeG36Controls'
    && !saved.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController'))return false
  return Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}`).some(name=>{
    const zone=saved.graph.blocks?.find(block=>block.name===name)
    return !zone?.inputs?.some(port=>port.name==='flow_override_code')
      || !zone.blocks?.some(block=>block.name==='flow_override_select')
      || !saved.graph.blocks?.some(block=>block.name===`${name}_flow_override_none`)
      || !saved.graph.connections?.some(edge=>edgeEndpoints(edge)[1]===`${name}.flow_override_code`)
  })
}

/** Add OBC airflow override wiring without replacing VAV control parameters. */
export function migrateG36FlowOverride(saved:Project,reference:Block):Project {
  if(!needsG36FlowOverrideMigration(saved))return saved
  const next=structuredClone(saved)
  next.graph.blocks??=[]
  next.graph.connections??=[]
  for(let index=1;index<=10;index++){
    const name=`zone_${String(index).padStart(2,'0')}`
    const zone=next.graph.blocks.find(block=>block.name===name)
    const template=reference.blocks?.find(block=>block.name===name)
    if(!zone||!template)throw new Error(`G36 reference lacks ${name}`)
    zone.blocks??=[]
    zone.inputs??=[]
    zone.connections??=[]
    if(!zone.inputs.some(port=>port.name==='flow_override_code')){
      const port=template.inputs?.find(port=>port.name==='flow_override_code')
      if(!port)throw new Error(`G36 reference lacks ${name}.flow_override_code`)
      zone.inputs.push(structuredClone(port))
    }
    for(const block of template.blocks??[]){
      if(!overrideBlocks(block.name)||zone.blocks.some(existing=>existing.name===block.name))continue
      zone.blocks.push(structuredClone(block))
    }
    const targets=new Set(zone.connections.map(edge=>edgeEndpoints(edge)[1]))
    for(const edge of template.connections??[]){
      const [from,to]=edgeEndpoints(edge)
      if(from==='flow_override_select.y'){
        const previous=zone.connections.find(existing=>{
          const [oldFrom,oldTo]=edgeEndpoints(existing)
          return oldFrom==='airflow_request_sum.y'&&oldTo===to
        })
        if(previous){previous.from='flow_override_select.y';continue}
      }
      if(!overrideBlocks(to.split('.')[0])&&!overrideBlocks(from.split('.')[0]))continue
      if(!targets.has(to)){zone.connections.push({from,to});targets.add(to)}
    }
    const sourceName=`${name}_flow_override_none`
    if(!next.graph.blocks.some(block=>block.name===sourceName)){
      const block=reference.blocks?.find(block=>block.name===sourceName)
      if(!block)throw new Error(`G36 reference lacks ${sourceName}`)
      next.graph.blocks.push(structuredClone(block))
    }
    if(!next.graph.connections.some(edge=>edgeEndpoints(edge)[1]===`${name}.flow_override_code`))
      next.graph.connections.push({from:`${sourceName}.y`,to:`${name}.flow_override_code`})
  }
  const fresh=layoutFor(next.graph)
  next.layout={...fresh,...saved.layout}
  for(let index=1;index<=10;index++){
    const name=`zone_${String(index).padStart(2,'0')}`
    const old=saved.layout?.[name]
    const oldDefault={x:index<=5?1660:3260,y:80+((index-1)%5)*280}
    if(old?.x===oldDefault.x&&old.y===oldDefault.y)next.layout[name]=fresh[name]
    const zone=next.graph.blocks?.find(block=>block.name===name)
    const inputIndex=zone?.inputs?.findIndex(port=>port.name==='flow_override_code')??0
    const position=next.layout[name]
    next.layout[`${name}_flow_override_none`]={x:position.x-400,y:position.y+Math.max(0,inputIndex)*28}
  }
  return next
}
