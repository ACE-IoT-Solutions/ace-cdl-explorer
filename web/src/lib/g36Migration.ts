import { edgeEndpoints, layoutFor, type Block, type Connection, type Project } from './types'

/** Move legacy harness-computed timing into editable CDL blocks without replacing user edits. */
export function migrateG36Timing(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls' || !saved.graph.inputs?.some(port => port.name === 'preconditioning_mode')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const referenceBlock = (parent: Block, name: string) => {
    const block = parent.blocks?.find(item => item.name === name)
    if (!block) throw new Error(`Current G36 example lacks ${name}`)
    return structuredClone(block)
  }
  const referencePort = (parent: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = parent[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    return structuredClone(port)
  }
  const connect = (parent: Block, from: string, to: string) => {
    if (!(parent.connections ?? []).some(edge => { const [a,b] = edgeEndpoints(edge); return a === from && b === to })) {
      ;(parent.connections ??= []).push({ from, to })
    }
  }
  const replaceSource = (edge: Connection, from: string) => {
    edge.from = from
    delete edge.from_block
    delete edge.from_output
  }
  const replaceTarget = (edge: Connection, to: string) => {
    edge.to = to
    delete edge.to_block
    delete edge.to_input
  }
  graph.inputs = graph.inputs?.filter(port => port.name !== 'preconditioning_mode') ?? []
  graph.inputs.push(referencePort(reference, 'inputs', 'seconds_until_occupied'))
  ;(graph.outputs ??= []).push(referencePort(reference, 'outputs', 'preconditioning_mode'))
  for (const name of ['preconditioning_horizon','within_preconditioning_horizon','schedule_unoccupied','preconditioning_enabled']) {
    if (!graph.blocks?.some(block => block.name === name)) (graph.blocks ??= []).push(referenceBlock(reference,name))
  }
  for (const [from,to] of [
    ['seconds_until_occupied','within_preconditioning_horizon.u1'],
    ['preconditioning_horizon.y','within_preconditioning_horizon.u2'],
    ['schedule_occupied','schedule_unoccupied.u'],
    ['within_preconditioning_horizon.y','preconditioning_enabled.u1'],
    ['schedule_unoccupied.y','preconditioning_enabled.u2'],
    ['preconditioning_enabled.y','preconditioning_mode'],
  ]) connect(graph,from,to)
  for (const edge of graph.connections ?? []) {
    const [from,to] = edgeEndpoints(edge)
    if (from === 'preconditioning_mode' && to !== 'preconditioning_mode') replaceSource(edge,'preconditioning_enabled.y')
  }
  for (let index=1;index<=10;index++) {
    const name=`zone_${String(index).padStart(2,'0')}`
    const zone=graph.blocks?.find(block=>block.name===name)
    const template=reference.blocks?.find(block=>block.name===name)
    if(!zone||!template)throw new Error(`G36 timing migration requires ${name}`)
    zone.inputs=(zone.inputs??[]).filter(port=>port.name!=='zone_occupied')
    zone.inputs.push(referencePort(template,'inputs','zone_presence_raw'))
    ;(zone.outputs??=[]).push(referencePort(template,'outputs','occupancy_confirmed'))
    for(const blockName of ['presence_delay','presence_inverse','vacancy_delay','occupancy_latch']){
      if(!zone.blocks?.some(block=>block.name===blockName))(zone.blocks??=[]).push(referenceBlock(template,blockName))
    }
    for(const edge of zone.connections??[]){
      if(edgeEndpoints(edge)[0]==='zone_occupied')replaceSource(edge,'occupancy_latch.y')
    }
    for(const [from,to] of [
      ['zone_presence_raw','presence_delay.u'],['zone_presence_raw','presence_inverse.u'],
      ['presence_inverse.y','vacancy_delay.u'],['presence_delay.y','occupancy_latch.u'],
      ['vacancy_delay.y','occupancy_latch.clr'],['occupancy_latch.y','occupancy_confirmed'],
    ])connect(zone,from,to)
    for(const edge of graph.connections??[]){
      if(edgeEndpoints(edge)[1]===`${name}.zone_occupied`)replaceTarget(edge,`${name}.zone_presence_raw`)
    }
    ;(graph.outputs??=[]).push(referencePort(reference,'outputs',`${name}_occupancy_confirmed`))
    connect(graph,`${name}.occupancy_confirmed`,`${name}_occupancy_confirmed`)
  }
  next.layout={...layoutFor(graph),...saved.layout}
  return next
}
