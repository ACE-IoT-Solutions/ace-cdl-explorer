import { edgeEndpoints, type Block, type Project } from './types'

/** Keep the minimum-discharge reheat exception within occupied mode. */
export function needsG36DischargeMinimumMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeG36Controls'
    && !saved.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController'))return false
  return (saved.graph.blocks??[]).some(zone=>zone.block_type==='VAVBoxController'
    && zone.blocks?.some(block=>block.name==='discharge_valve_enable')
    && (!zone.blocks?.some(block=>block.name==='discharge_minimum_occupied')
      ||!zone.blocks?.find(block=>block.name==='discharge_below_minimum')?.parameters?.some(parameter=>parameter.name==='h')))
}

export function migrateG36DischargeMinimum(saved:Project,reference:Block):Project {
  if(!needsG36DischargeMinimumMigration(saved))return saved
  const next=structuredClone(saved)
  for(const zone of next.graph.blocks??[]){
    if(zone.block_type!=='VAVBoxController'||!zone.blocks?.some(block=>block.name==='discharge_valve_enable'))continue
    const template=reference.blocks?.find(block=>block.name===zone.name)
    const comparator=zone.blocks.find(block=>block.name==='discharge_below_minimum')
    const templateComparator=template?.blocks?.find(block=>block.name==='discharge_below_minimum')
    if(!comparator||!templateComparator)throw new Error(`G36 reference lacks ${zone.name}.discharge_below_minimum`)
    comparator.parameters??=[]
    if(!comparator.parameters.some(parameter=>parameter.name==='h')){
      const hysteresis=templateComparator.parameters?.find(parameter=>parameter.name==='h')
      if(!hysteresis)throw new Error(`G36 reference lacks ${zone.name} discharge hysteresis`)
      comparator.parameters.push(structuredClone(hysteresis))
    }
    if(!zone.blocks.some(block=>block.name==='discharge_minimum_occupied')){
      const gate=template?.blocks?.find(block=>block.name==='discharge_minimum_occupied')
      if(!gate)throw new Error(`G36 reference lacks ${zone.name}.discharge_minimum_occupied`)
      zone.blocks.push(structuredClone(gate))
      zone.connections??=[]
      const old=zone.connections.find(edge=>edgeEndpoints(edge)[1]==='discharge_valve_enable.u2')
      if(!old)throw new Error(`${zone.name} lacks discharge valve enable wiring`)
      const minimumCondition=edgeEndpoints(old)[0]
      old.from='discharge_minimum_occupied.y'
      zone.connections.push(
        {from:minimumCondition,to:'discharge_minimum_occupied.u1'},
        {from:'local_occupied.y',to:'discharge_minimum_occupied.u2'},
      )
    }
  }
  return next
}
