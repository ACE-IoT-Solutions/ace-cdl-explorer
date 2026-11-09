import { edgeEndpoints, type Block, type Project } from './types'

const additions=new Set(['normal_cooling_signal','normal_cooling_limit','normal_heating_signal','ventilation_override_or_disabled','normal_airflow_sequence','normal_warmup_select'])

/** Replace the damper-command airflow proxy with the editable OBC normal-flow sequence. */
export function needsG36NormalFlowMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeG36Controls'
    && !saved.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController'))return false
  return (saved.graph.blocks??[]).some(zone=>zone.block_type==='VAVBoxController'
    && ([...additions].some(name=>!zone.blocks?.some(block=>block.name===name))
      ||!zone.connections?.some(edge=>{
        const [from,to]=edgeEndpoints(edge)
        return from==='normal_warmup_select.y'&&to==='flow_override_select.u2'
      })))
}

export function migrateG36NormalFlow(saved:Project,reference:Block):Project {
  if(!needsG36NormalFlowMigration(saved))return saved
  const next=structuredClone(saved)
  for(const zone of next.graph.blocks??[]){
    if(zone.block_type!=='VAVBoxController')continue
    const template=reference.blocks?.find(block=>block.name===zone.name)
    if(!template)throw new Error(`G36 reference lacks ${zone.name}`)
    zone.blocks??=[]
    zone.connections??=[]
    for(const name of additions){
      if(zone.blocks.some(block=>block.name===name))continue
      const block=template.blocks?.find(block=>block.name===name)
      if(!block)throw new Error(`G36 reference lacks ${zone.name}.${name}`)
      zone.blocks.push(structuredClone(block))
    }
    // Preserve a prior nondefault flow-span edit as a nominal maximum when the
    // newer maximum has not already been tuned independently.
    const oldGain=zone.blocks.find(block=>block.name==='flow_span')?.parameters?.find(parameter=>parameter.name==='k')?.value
    if(typeof oldGain==='number'&&Number.isFinite(oldGain)&&Math.abs(oldGain-0.75)>1e-9){
      for(const name of ['flow_override_cooling_max','flow_override_heating_max']){
        const maximum=zone.blocks.find(block=>block.name===name)?.parameters?.find(parameter=>parameter.name==='k')
        if(maximum?.value===0.95)maximum.value=Math.max(0,0.2+oldGain)
      }
    }
    const target='flow_override_select.u2'
    const previous=zone.connections.find(edge=>edgeEndpoints(edge)[1]===target)
    if(!previous)throw new Error(`${zone.name} lacks flow override normal-input wiring`)
    previous.from='normal_warmup_select.y'
    const heatingGain=zone.connections.find(edge=>edgeEndpoints(edge)[1]==='heating_damper_gain.u')
    if(!heatingGain)throw new Error(`${zone.name} lacks editable heating gain input`)
    heatingGain.from='heat_pi.y'
    const areaEligibility=zone.connections.find(edge=>edgeEndpoints(edge)[1]==='ventilation_area_presence_or_required.u2')
    if(!areaEligibility)throw new Error(`${zone.name} lacks area minimum eligibility input`)
    areaEligibility.from='ventilation_override_or_disabled.y'
    const existing=new Set(zone.connections.map(edge=>edgeEndpoints(edge)[1]))
    for(const edge of template.connections??[]){
      const [from,to]=edgeEndpoints(edge)
      if(!additions.has(to.split('.')[0]))continue
      if(!existing.has(to)){zone.connections.push({from,to});existing.add(to)}
    }
  }
  return next
}
