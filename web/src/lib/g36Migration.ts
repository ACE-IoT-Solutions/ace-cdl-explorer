import { edgeEndpoints, layoutFor, type Block, type Connection, type Project } from './types'

/** Correct the shipped two-minute vacancy default to the documented five minutes. */
export function migrateG36VacancyDelay(saved: Project): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const affected = (saved.graph.blocks ?? []).filter(block => block.block_type === 'VAVBoxController')
    .some(zone => zone.blocks?.find(block => block.name === 'vacancy_delay')?.parameters?.find(parameter => parameter.name === 'delayTime')?.value === 120)
  if (!affected) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const delay = zone.blocks?.find(block => block.name === 'vacancy_delay')?.parameters?.find(parameter => parameter.name === 'delayTime')
    if (delay?.value === 120) delay.value = 300
  }
  return next
}

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

/** Add measured terminal feedback and editable reheat limiting to earlier G36 projects. */
export function migrateG36TerminalFeedback(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.inputs?.some(port => port.name === 'zone_01_measured_airflow')
    && saved.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.some(block => block.name === 'heating_damper_select')) return saved
  const next = structuredClone(saved)
  const blockNames = new Set([
    'maximum_discharge_temp', 'discharge_headroom', 'reheat_capacity_per_flow',
    'reheat_flow_cap', 'reheat_cap_limit', 'reheat_capped', 'low_flow_fraction',
    'low_flow_threshold', 'low_flow_detected', 'flow_alarm_enable_threshold',
    'flow_requested', 'low_flow_condition', 'low_flow_delay',
    'high_discharge_detected', 'high_discharge_delay',
    'heating_damper_gain', 'heating_damper_offset', 'heating_damper_limit', 'heating_damper_select',
  ])
  function copyPort(target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  function copyEdge(target: Block, from: string, to: string) {
    if (!(target.connections ?? []).some(edge => {
      const [a, b] = edgeEndpoints(edge)
      return a === from && b === to
    })) (target.connections ??= []).push({ from, to })
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 terminal migration requires ${name}`)
    for (const point of ['measured_airflow', 'discharge_temp']) {
      copyPort(next.graph, reference, 'inputs', `${name}_${point}`)
      copyPort(zone, template, 'inputs', point)
    }
    for (const point of ['low_airflow_alarm', 'high_discharge_alarm']) {
      copyPort(next.graph, reference, 'outputs', `${name}_${point}`)
      copyPort(zone, template, 'outputs', point)
    }
    for (const child of template.blocks ?? []) {
      if (blockNames.has(child.name) && !zone.blocks?.some(block => block.name === child.name)) {
        (zone.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'reheat_capped.y' && to === 'reheat_valve_position') continue
      if (from === 'heating_damper_select.y' && ['damper_position', 'flow_span.u'].includes(to)) continue
      const fromBlock = from.split('.')[0]
      const toBlock = to.split('.')[0]
      if (blockNames.has(fromBlock) || blockNames.has(toBlock) || ['measured_airflow', 'discharge_temp', 'low_airflow_alarm', 'high_discharge_alarm'].includes(from) || ['measured_airflow', 'discharge_temp', 'low_airflow_alarm', 'high_discharge_alarm'].includes(to)) {
        copyEdge(zone, from, to)
      }
    }
    for (const edge of zone.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'reheat_select.y' && to === 'reheat_valve_position') {
        edge.from = 'reheat_capped.y'
        delete edge.from_block
        delete edge.from_output
      }
      if (from === 'damper_select.y' && ['damper_position', 'flow_span.u'].includes(to)) {
        edge.from = 'heating_damper_select.y'
        delete edge.from_block
        delete edge.from_output
      }
    }
    if (!(zone.connections ?? []).some(edge => edgeEndpoints(edge)[1] === 'reheat_valve_position')) {
      copyEdge(zone, 'reheat_capped.y', 'reheat_valve_position')
    }
    for (const target of ['damper_position', 'flow_span.u']) {
      if (!(zone.connections ?? []).some(edge => edgeEndpoints(edge)[1] === target)) {
        copyEdge(zone, 'heating_damper_select.y', target)
      }
    }
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from.startsWith(`${name}_measured_airflow`) || from.startsWith(`${name}_discharge_temp`) || from.startsWith(`${name}.low_airflow_alarm`) || from.startsWith(`${name}.high_discharge_alarm`)) {
        copyEdge(next.graph, from, to)
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}

/** Add integer zone pressure requests and AHU pressure/ventilation feedback. */
export function migrateG36AhuFeedback(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.inputs?.some(port => port.name === 'ahu_1_outdoor_airflow')
    && saved.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.some(block => block.name === 'fan_enable')) return saved
  const next = structuredClone(saved)
  const copyPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const connect = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${from}|${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  const zoneBlocks = new Set([
    'pressure_request_damper_threshold', 'damper_nearly_open', 'pressure_request_flow_fraction',
    'pressure_request_low_flow', 'pressure_request_condition', 'pressure_request_delay',
    'pressure_request_integer',
  ])
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 pressure migration requires ${name}`)
    copyPort(zone, template, 'outputs', 'pressure_reset_request')
    copyPort(next.graph, reference, 'outputs', `${name}_pressure_reset_request`)
    for (const child of template.blocks ?? []) {
      if (zoneBlocks.has(child.name) && !zone.blocks?.some(block => block.name === child.name)) {
        (zone.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (zoneBlocks.has(from.split('.')[0]) || zoneBlocks.has(to.split('.')[0]) || to === 'pressure_reset_request') connect(zone, from, to)
    }
    connect(next.graph, `${name}.pressure_reset_request`, `${name}_pressure_reset_request`)
  }
  const ahuBlocks = new Set([
    'request_sum_1', 'request_sum_2', 'request_sum_3', 'request_sum_4',
    'ignored_request_count', 'request_count_high', 'request_delay',
    'low_pressure_target', 'high_pressure_target', 'pressure_target_select',
    'duct_pressure_pi', 'fan_feedforward_max', 'fan_request_threshold',
    'fan_requested', 'fan_off', 'fan_enable', 'minimum_oa_flow_gain',
    'minimum_oa_pi', 'minimum_oa_command',
  ])
  for (let ahu = 1; ahu <= 2; ahu++) {
    const name = `ahu_${ahu}`
    const controller = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!controller || !template) throw new Error(`G36 AHU migration requires ${name}`)
    for (const point of ['duct_pressure', 'outdoor_airflow']) {
      copyPort(next.graph, reference, 'inputs', `${name}_${point}`)
      copyPort(controller, template, 'inputs', point)
      connect(next.graph, `${name}_${point}`, `${name}.${point}`)
    }
    for (const point of ['pressure_request_count', 'duct_pressure_setpoint', 'minimum_outdoor_airflow']) {
      copyPort(next.graph, reference, 'outputs', `${name}_${point}`)
      copyPort(controller, template, 'outputs', point)
      connect(next.graph, `${name}.${point}`, `${name}_${point}`)
    }
    for (let index = (ahu - 1) * 5 + 1; index <= ahu * 5; index++) {
      const zone = `zone_${String(index).padStart(2, '0')}`
      copyPort(controller, template, 'inputs', `${zone}_pressure_request`)
      connect(next.graph, `${zone}.pressure_reset_request`, `${name}.${zone}_pressure_request`)
    }
    for (const child of template.blocks ?? []) {
      if (ahuBlocks.has(child.name) && !controller.blocks?.some(block => block.name === child.name)) {
        (controller.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'fan_speed' || to === 'outdoor_air_fraction') continue
      if (ahuBlocks.has(from.split('.')[0]) || ahuBlocks.has(to.split('.')[0])
        || ['duct_pressure', 'outdoor_airflow'].includes(from)
        || ['pressure_request_count', 'duct_pressure_setpoint', 'minimum_outdoor_airflow'].includes(to)
        || from.endsWith('_pressure_request')) connect(controller, from, to)
    }
    for (const edge of controller.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === 'fan_limit.y' && to === 'fan_speed') {
        edge.from = 'fan_enable.y'; delete edge.from_block; delete edge.from_output
      }
      if (from === 'outdoor_air_select.y' && to === 'outdoor_air_fraction') {
        edge.from = 'minimum_oa_command.y'; delete edge.from_block; delete edge.from_output
      }
    }
    if (!(controller.connections ?? []).some(edge => edgeEndpoints(edge)[1] === 'fan_speed')) connect(controller, 'fan_enable.y', 'fan_speed')
    if (!(controller.connections ?? []).some(edge => edgeEndpoints(edge)[1] === 'outdoor_air_fraction')) connect(controller, 'minimum_oa_command.y', 'outdoor_air_fraction')
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}

/** Replace the early two-level AHU pressure reset with a sampled CDL feedback graph. */
export function migrateG36PressureTrim(saved: Project, reference: Block): Project {
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous'))return saved
  const next=structuredClone(saved)
  const retired=new Set(['request_count_high','pressure_target_select'])
  const pressureTrimBlocks=new Set([
    'ignored_request_count','request_count_real','ignored_request_real','excess_request_count',
    'zero_requests','positive_request_count','response_per_request','maximum_response',
    'bounded_response','trim_amount','net_pressure_reset','request_delay',
    'initial_pressure_target','low_pressure_target','high_pressure_target',
    'pressure_previous','pressure_target_sum','pressure_above_minimum',
    'pressure_below_maximum','pressure_candidate','pressure_target_output',
  ])
  for(let index=1;index<=2;index++){
    const name=`ahu_${index}`
    const ahu=next.graph.blocks?.find(block=>block.name===name)
    const template=reference.blocks?.find(block=>block.name===name)
    if(!ahu||!template)throw new Error(`G36 pressure trim migration requires ${name}`)
    ahu.blocks=ahu.blocks?.filter(block=>!retired.has(block.name))??[]
    ahu.connections=ahu.connections?.filter(edge=>{
      const [from,to]=edgeEndpoints(edge)
      return !retired.has(from.split('.')[0])&&!retired.has(to.split('.')[0])
        &&to!=='request_delay.u'&&to!=='duct_pressure_setpoint'&&to!=='duct_pressure_pi.u_s'
    })??[]
    const low=ahu.blocks.find(block=>block.name==='low_pressure_target')?.parameters?.find(parameter=>parameter.name==='k')
    if(low?.value===0.35)low.value=0.25
    const delay=ahu.blocks.find(block=>block.name==='request_delay')
    if(delay)delay.notes=template.blocks?.find(block=>block.name==='request_delay')?.notes
    const added=new Set<string>()
    for(const block of template.blocks??[]){
      if(pressureTrimBlocks.has(block.name)&&!ahu.blocks.some(existing=>existing.name===block.name)){
        ahu.blocks.push(structuredClone(block));added.add(block.name)
      }
    }
    for(const edge of template.connections??[]){
      const [from,to]=edgeEndpoints(edge)
      if(added.has(from.split('.')[0])||added.has(to.split('.')[0])
        ||['request_delay.u','duct_pressure_setpoint','duct_pressure_pi.u_s'].includes(to)){
        if(!ahu.connections.some(existing=>edgeEndpoints(existing).join('|')===`${from}|${to}`))ahu.connections.push(structuredClone(edge))
      }
    }
  }
  next.layout={...layoutFor(next.graph),...saved.layout}
  return next
}

/** Add measured mixed-air feedback and editable economizer modulation to saved G36 examples. */
export function migrateG36Economizer(saved: Project, reference: Block): Project {
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='economizer_mixed_air_pi'))return saved
  const next=structuredClone(saved)
  for(let index=1;index<=2;index++){
    const name=`ahu_${index}`
    const ahu=next.graph.blocks?.find(block=>block.name===name)
    const template=reference.blocks?.find(block=>block.name===name)
    if(!ahu||!template)throw new Error(`G36 economizer migration requires ${name}`)
    const topPort=reference.inputs?.find(port=>port.name===`${name}_mixed_air_temp`)
    const innerPort=template.inputs?.find(port=>port.name==='mixed_air_temp')
    const pi=template.blocks?.find(block=>block.name==='economizer_mixed_air_pi')
    if(!topPort||!innerPort||!pi)throw new Error('Current G36 example lacks mixed-air feedback')
    if(!next.graph.inputs?.some(port=>port.name===topPort.name))(next.graph.inputs??=[]).push(structuredClone(topPort))
    if(!ahu.inputs?.some(port=>port.name===innerPort.name))(ahu.inputs??=[]).push(structuredClone(innerPort))
    if(!(next.graph.connections??[]).some(edge=>edgeEndpoints(edge).join('|')===`${name}_mixed_air_temp|${name}.mixed_air_temp`)){
      (next.graph.connections??=[]).push({from:`${name}_mixed_air_temp`,to:`${name}.mixed_air_temp`})
    }
    const oldMaximum=ahu.blocks?.find(block=>block.name==='free_cooling_outdoor_air')?.parameters?.find(parameter=>parameter.name==='k')?.value
    const migratedPi=structuredClone(pi)
    if(typeof oldMaximum==='number'&&oldMaximum>0&&oldMaximum<=1){
      const maximum=migratedPi.parameters?.find(parameter=>parameter.name==='yMax')
      if(maximum)maximum.value=oldMaximum
    }
    ahu.blocks=ahu.blocks?.filter(block=>!['free_cooling_outdoor_air','outdoor_above_supply_target'].includes(block.name))??[]
    ahu.blocks.push(migratedPi)
    ahu.connections=ahu.connections?.filter(edge=>{
      const [from,to]=edgeEndpoints(edge)
      return !['free_cooling_outdoor_air','outdoor_above_supply_target'].includes(from.split('.')[0])
        &&!['free_cooling_outdoor_air','outdoor_above_supply_target'].includes(to.split('.')[0])
        &&to!=='economizer_temperature_ok.u2'&&to!=='outdoor_air_select.u1'
    })??[]
    for(const [from,to] of [
      ['supply_reset.y','economizer_mixed_air_pi.u_s'],
      ['mixed_air_temp','economizer_mixed_air_pi.u_m'],
      ['economizer_mixed_air_pi.y','outdoor_air_select.u1'],
      ['fan_requested.y','economizer_temperature_ok.u2'],
    ])ahu.connections.push({from,to})
    const enable=ahu.blocks.find(block=>block.name==='economizer_temperature_ok')
    const select=ahu.blocks.find(block=>block.name==='outdoor_air_select')
    if(enable)enable.notes=template.blocks?.find(block=>block.name===enable.name)?.notes
    if(select)select.notes=template.blocks?.find(block=>block.name===select.name)?.notes
  }
  next.layout={...layoutFor(next.graph),...saved.layout}
  return next
}
