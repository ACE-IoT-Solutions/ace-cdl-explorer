import { edgeEndpoints, type Block, type Project } from './types'

/** Keep the minimum-discharge reheat exception within occupied mode. */
export function needsG36DischargeMinimumMigration(saved:Project):boolean {
  if(saved.graph.name!=='FourPipeG36Controls'
    && !saved.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController'))return false
  return (saved.graph.blocks??[]).some(zone=>zone.block_type==='VAVBoxController'
    && zone.blocks?.some(block=>block.name==='discharge_valve_enable')
    && !zone.blocks?.some(block=>block.name==='discharge_minimum_occupied'))
}

export function migrateG36DischargeMinimum(saved:Project,reference:Block):Project {
  if(!needsG36DischargeMinimumMigration(saved))return saved
  const next=structuredClone(saved)
  for(const zone of next.graph.blocks??[]){
    if(zone.block_type!=='VAVBoxController'||!zone.blocks?.some(block=>block.name==='discharge_valve_enable')
      ||zone.blocks.some(block=>block.name==='discharge_minimum_occupied'))continue
    const template=reference.blocks?.find(block=>block.name===zone.name)
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
  return next
}
