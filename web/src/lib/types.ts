export interface Connector { name: string; type: 'Real' | 'Boolean' | 'Integer' | 'String'; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string; label?: string; brick_class?: string }
export interface Parameter { name: string; type?: string; value: number | boolean | string; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string }
export interface Equation { lhs: string; rhs: string }
export interface Connection { from?: string; to?: string; from_block?: string; from_output?: string; to_block?: string; to_input?: string }
export interface Block {
  name: string; block_type: string; category?: string; description?: string; notes?: string;
  inputs?: Connector[]; outputs?: Connector[]; parameters?: Parameter[]; equations?: Equation[];
  blocks?: Block[]; connections?: Connection[]
}
export interface HeatPumpPerformancePoint { outdoor_c: number; capacity_fraction: number; heating_cop: number; cooling_cop: number }
export interface EquipmentOutage { equipment: 'heat_pump' | 'boiler' | 'chiller'; start_hour: number; duration_hours: number }
export interface FanFault { equipment: 'ahu_1_stuck_off' | 'ahu_1_stuck_on' | 'ahu_2_stuck_off' | 'ahu_2_stuck_on' | 'ahu_1_speed_capped' | 'ahu_2_speed_capped'; start_hour: number; duration_hours: number }
export interface TerminalFault { zone: number; kind: 'damper_stuck_open' | 'reheat_valve_leak'; start_hour: number; duration_hours: number }
export interface WindowOpening { zone: number; start_hour: number; duration_hours: number }
export interface ZoneOverride { zone: number; start_hour: number; duration_hours: number }
export interface EquipmentStartupSeconds { heat_pump: number; boiler: number; chiller: number }
export interface Scenario { zone_ua_kw_per_k: number; zone_capacity_kwh_per_k: number; heat_pump_fraction?: number /* legacy projects only; migrated into plant CDL */; heat_pump_capacity_kw: number; heat_pump_performance: HeatPumpPerformancePoint[]; equipment_outages: EquipmentOutage[]; equipment_start_failures: EquipmentOutage[]; fan_faults: FanFault[]; terminal_faults: TerminalFault[]; window_openings: WindowOpening[]; occupancy_overrides: ZoneOverride[]; equipment_startup_seconds: EquipmentStartupSeconds; heat_pump_fault_reset_hour: number; boiler_fault_reset_hour: number; chiller_fault_reset_hour: number; boiler_capacity_kw: number; chiller_capacity_kw: number; step_seconds: 15 | 30 | 60 | 300 | 900; freeze_test_ahu: number; freeze_test_start_hour: number; freeze_test_duration_hours: number; freeze_test_reset_hour: number }
export interface Project { version: 1; name: string; graph: Block; scenario: Scenario; layout: Record<string, { x: number; y: number }> }
export interface Row {
  index: number; outdoor_c: number; avg_zone_c: number; zone_c: number[];
  schedule_occupied?: number; preconditioning_mode?: number; zone_occupied?: number[]; zone_presence_raw?: number[];
  zone_airflow?: number[]; zone_discharge_c?: number[];
  zone_actual_damper_position?: number[]; zone_actual_reheat_valve_position?: number[];
  cool_kw: number; heat_kw: number; cool_request_kw: number; heat_request_kw: number;
  electric_kw: number; gas_kw: number; requested_airflow: number; delivered_airflow: number;
  ahu_1_supply_actual_c: number; ahu_2_supply_actual_c: number;
  ahu_1_supply_setpoint_c: number; ahu_2_supply_setpoint_c: number;
  ahu_1_duct_pressure: number; ahu_2_duct_pressure: number;
  ahu_1_outdoor_air_fraction: number; ahu_2_outdoor_air_fraction: number;
  ahu_1_intake_opening?: number; ahu_2_intake_opening?: number;
  ahu_1_actual_fan_speed?: number; ahu_2_actual_fan_speed?: number;
  ahu_1_heating_coil_valve_position?: number; ahu_2_heating_coil_valve_position?: number;
  ahu_1_cooling_coil_valve_position?: number; ahu_2_cooling_coil_valve_position?: number;
  ahu_1_heating_coil_request_kw?: number; ahu_2_heating_coil_request_kw?: number;
  ahu_1_cooling_coil_request_kw?: number; ahu_2_cooling_coil_request_kw?: number;
  ahu_1_heating_coil_delivered_kw?: number; ahu_2_heating_coil_delivered_kw?: number;
  ahu_1_cooling_coil_delivered_kw?: number; ahu_2_cooling_coil_delivered_kw?: number;
  ahu_1_outdoor_airflow: number; ahu_2_outdoor_airflow: number;
  ahu_1_mixed_air_c: number; ahu_2_mixed_air_c: number;
  heat_pump_heating_used_kw: number; heat_pump_cooling_used_kw: number;
  heat_pump_electric_kw?: number; heat_pump_ambient_exchange_kw?: number;
  heat_pump_available_capacity_kw?: number; heat_pump_heating_cop?: number; heat_pump_cooling_cop?: number;
  heat_pump_available?: number; boiler_available?: number; chiller_available?: number;
  heat_pump_running?: number; boiler_running?: number; chiller_running?: number; plant_heat_pump_start_fault?: number; plant_boiler_start_fault?: number; plant_chiller_start_fault?: number;
  boiler_used_kw: number; chiller_used_kw: number; traces?: Record<string,number>
  hot_water_supply_temp_c?: number; hot_water_supply_setpoint_c?: number; hot_water_return_temp_c?: number;
  chilled_water_supply_temp_c?: number; chilled_water_supply_setpoint_c?: number; chilled_water_return_temp_c?: number;
  hot_water_flow_kg_s?: number; chilled_water_flow_kg_s?: number;
  hot_water_differential_pressure_pa?: number; chilled_water_differential_pressure_pa?: number;
  hot_water_pump_speed?: number; chilled_water_pump_speed?: number;
  hot_water_enable?: number; chilled_water_enable?: number; water_pump_kw?: number;
  hot_water_plant_proven?: number;
}
export interface Summary { electricity_kwh: number; gas_kwh: number; unmet_zone_hours: number; occupied_unmet_zone_hours: number }
export const defaultScenario: Scenario = { zone_ua_kw_per_k: 0.15, zone_capacity_kwh_per_k: 5, heat_pump_capacity_kw: 80, heat_pump_performance: [{outdoor_c:-15,capacity_fraction:0.55,heating_cop:2,cooling_cop:3},{outdoor_c:10,capacity_fraction:1,heating_cop:3.2,cooling_cop:3.8},{outdoor_c:35,capacity_fraction:0.8,heating_cop:2.7,cooling_cop:3}], equipment_outages: [], equipment_start_failures: [], fan_faults: [], terminal_faults: [], window_openings: [], occupancy_overrides: [], equipment_startup_seconds: {heat_pump:0,boiler:0,chiller:0}, heat_pump_fault_reset_hour: -1, boiler_fault_reset_hour: -1, chiller_fault_reset_hour: -1, boiler_capacity_kw: 100, chiller_capacity_kw: 100, step_seconds: 900, freeze_test_ahu: 0, freeze_test_start_hour: -1, freeze_test_duration_hours: 0, freeze_test_reset_hour: -1 }

/** Smallest step that aligns a requested maximum with all sampled CDL blocks. */
export function effectiveStepForGraph(graph:Block, requested:number):number {
  const gcd=(a:number,b:number):number=>{while(b){[a,b]=[b,a%b]}return a}
  let step=requested
  const visit=(block:Block)=>{
    if(block.block_type.split('.').at(-1)==='UnitDelay'){
      const period=block.parameters?.find(parameter=>parameter.name==='samplePeriod')?.value
      if(typeof period==='number'&&Number.isInteger(period)&&period>0)step=gcd(step,period)
    }
    for(const child of block.blocks??[])visit(child)
  }
  visit(graph)
  return step
}

export function edgeEndpoints(edge: Connection): [string, string] {
  const source = edge.from ?? (edge.from_output ? `${edge.from_block}.${edge.from_output}` : edge.from_block ?? '')
  const target = edge.to ?? (edge.to_input ? `${edge.to_block}.${edge.to_input}` : edge.to_block ?? '')
  return [source, target]
}

export function layoutFor(graph: Block, legacy: boolean | 'previous' | 'depth24' = false): Record<string, { x: number; y: number }> {
  const layout: Record<string, { x: number; y: number }> = {}
  function visit(block: Block, prefix: string) {
    const children=block.blocks??[]
    const isUnitDelay=(child:Block)=>child.block_type.split('.').at(-1)==='UnitDelay'
    if(prefix==='' && legacy!==true && block.block_type==='BuildingController'){
      children.forEach((child,index)=>{
        const zone=Number(child.name.match(/^zone_(\d{2})$/)?.[1]??0)
        if(legacy==='previous')layout[child.name]=child.name==='plant'?{x:60,y:80}:child.name==='ahu_1'?{x:460,y:80}:child.name==='ahu_2'?{x:1260,y:80}:zone>0?{x:zone<=5?840:1640,y:80+((zone-1)%5)*240}:{x:60+(index%4)*400,y:80+Math.floor(index/4)*240}
        else {const spacing=legacy==='depth24'?240:280;layout[child.name]=child.name==='plant'?{x:60,y:80}:child.name==='ahu_1'?{x:860,y:80}:child.name==='ahu_2'?{x:2460,y:80}:zone>0?{x:zone<=5?1660:3260,y:80+((zone-1)%5)*spacing}:{x:60+(index%4)*800,y:80+Math.floor(index/4)*spacing}}
      })
    }else if(legacy===false||legacy==='depth24'){
      const names=new Set(children.map(child=>child.name))
      const byName=new Map(children.map(child=>[child.name,child]))
      const dependencies=new Map(children.map(child=>[child.name,isUnitDelay(child)?[]:(block.connections??[]).filter(edge=>edgeEndpoints(edge)[1].split('.')[0]===child.name).map(edge=>edgeEndpoints(edge)[0].split('.')[0]).filter(name=>names.has(name))]))
      const levels=new Map<string,number>()
      let pending=[...children]
      while(pending.length){
        const ready=pending.filter(child=>(dependencies.get(child.name)??[]).every(name=>levels.has(name)))
        if(!ready.length)break
        for(const child of ready)levels.set(child.name,Math.max(0,...(dependencies.get(child.name)??[]).map(name=>(levels.get(name)??-1)+1)))
        pending=pending.filter(child=>!levels.has(child.name))
      }
      for(const child of pending)levels.set(child.name,0)
      const height=(child:Block)=>Math.max(140,68+Math.max(child.inputs?.length??0,child.outputs?.length??0)*(legacy==='depth24'?24:28)+(child.category==='composite'?30:0)+(child.category==='composite'?0:38))
      const maxLevel=Math.max(0,...levels.values())
      for(let level=0;level<=maxLevel;level++){
        const desired=(child:Block)=>{
          const previous=(dependencies.get(child.name)??[]).filter(name=>(levels.get(name)??0)<level)
          if(!previous.length)return 80+children.indexOf(child)*20
          return previous.reduce((sum,name)=>sum+(layout[`${prefix}${name}`]?.y??80)+height(byName.get(name)!)/2,0)/previous.length-height(child)/2
        }
        const layer=children.filter(child=>levels.get(child.name)===level).sort((a,b)=>desired(a)-desired(b)||children.indexOf(a)-children.indexOf(b))
        let bottom=80
        for(const child of layer){const y=Math.max(bottom,desired(child));layout[`${prefix}${child.name}`]={x:60+level*560,y};bottom=y+height(child)+56}
      }
    }else if(prefix===''){
      children.forEach((child,index)=>{layout[`${prefix}${child.name}`]={x:60+(index%5)*(legacy===true?250:360),y:80+Math.floor(index/5)*(legacy===true?270:240)}})
    }else{
      const names=new Set(children.map(c=>c.name))
      const levels=new Map<string,number>()
      let pending=[...children]
      while(pending.length){
        const ready=pending.filter(child=>isUnitDelay(child)||(block.connections??[]).every(edge=>{const [source,target]=edgeEndpoints(edge);return target.split('.')[0]!==child.name||!names.has(source.split('.')[0])||levels.has(source.split('.')[0])}))
        if(!ready.length)break
        for(const child of ready){const dependencies=isUnitDelay(child)?[]:(block.connections??[]).filter(edge=>edgeEndpoints(edge)[1].split('.')[0]===child.name).map(edge=>edgeEndpoints(edge)[0].split('.')[0]).filter(name=>names.has(name));levels.set(child.name,Math.max(0,...dependencies.map(name=>(levels.get(name)??-1)+1)))}
        pending=pending.filter(child=>!levels.has(child.name))
      }
      const rowByLevel=new Map<number,number>()
      children.forEach((child,index)=>{const level=levels.get(child.name)??index%5;const row=rowByLevel.get(level)??0;rowByLevel.set(level,row+1);layout[`${prefix}${child.name}`]={x:60+level*(legacy===true?250:360),y:80+row*(legacy===true?190:240)}})
    }
    children.forEach(child=>visit(child,`${prefix}${child.name}.`))
  }
  visit(graph, '')
  return layout
}

export const palette: Array<{ type: string; label: string; inputs: Connector[]; outputs: Connector[]; parameters: Parameter[] }> = [
  { type: 'Composite', label: 'Composite', inputs: [], outputs: [], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Sources.Constant', label: 'Constant', inputs: [], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Add', label: 'Add', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k1', value: 1 }, { name: 'k2', value: 1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Integers.Add', label: 'Integer add', inputs: [{ name: 'u1', type: 'Integer' }, { name: 'u2', type: 'Integer' }], outputs: [{ name: 'y', type: 'Integer' }], parameters: [] },
  { type: 'CDL.Reals.Min', label: 'Min', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Reals.Max', label: 'Max', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger', label: 'Boolean to integer', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Integer' }], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Conversions.IntegerToReal', label: 'Integer to real', inputs: [{ name: 'u', type: 'Integer' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Discrete.UnitDelay', label: 'Sampled unit delay', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'samplePeriod', value: 120 }, { name: 'y_start', value: 0 }] },
  { type: 'Buildings.Controls.OBC.CDL.Discrete.TriggeredSampler', label: 'Triggered sampler', inputs: [{ name: 'u', type: 'Real' }, { name: 'trigger', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'y_start', value: 0 }] },
  { type: 'Buildings.Controls.OBC.CDL.Discrete.TriggeredMovingMean', label: 'Triggered moving mean', inputs: [{ name: 'u', type: 'Real' }, { name: 'trigger', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'n', value: 3 }] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.Edge', label: 'Rising edge', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [{ name: 'pre_u_start', value: false, type: 'Boolean' }] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.FallingEdge', label: 'Falling edge', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [{ name: 'pre_u_start', value: false, type: 'Boolean' }] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.Timer', label: 'Timer', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Real', unit: 's' }, { name: 'passed', type: 'Boolean' }], parameters: [{ name: 't', value: 3600, unit: 's' }] },
  { type: 'Subtract', label: 'Subtract', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Multiply', label: 'Multiply', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Greater', label: 'Greater', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'Less', label: 'Less', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'And', label: 'And', inputs: [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'Or', label: 'Or', inputs: [{ name: 'u1', type: 'Boolean' }, { name: 'u2', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'Not', label: 'Not', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'CDL.Reals.MultiplyByParameter', label: 'Gain', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Limiter', label: 'Limiter', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'uMin', value: 0 }, { name: 'uMax', value: 1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.LimPID', label: 'PI control', inputs: [{ name: 'u_s', type: 'Real' }, { name: 'u_m', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 0.5 }, { name: 'Ti', value: 60 }, { name: 'yMin', value: 0 }, { name: 'yMax', value: 1 }, { name: 'reverseActing', value: false, type: 'Boolean' }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Filter', label: 'Filter', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'T', value: 10 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.LimRateLimiter', label: 'Rate limit', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'riseRate', value: 0.1 }, { name: 'fallRate', value: 0.1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Hysteresis', label: 'Hysteresis', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [{ name: 'uLow', value: -1 }, { name: 'uHigh', value: 1 }] },
  { type: 'Buildings.Controls.OBC.CDL.Continuous.Switch', label: 'Switch', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }, { name: 'u3', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.TrueDelay', label: 'True delay', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [{ name: 'delayTime', value: 60, unit: 's' }, { name: 'delayOnInit', value: true, type: 'Boolean' }] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.TrueFalseHold', label: 'True/false hold', inputs: [{ name: 'u', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [{ name: 'trueHoldDuration', value: 60, unit: 's' }, { name: 'falseHoldDuration', value: 60, unit: 's' }] },
  { type: 'Buildings.Controls.OBC.CDL.Logical.Latch', label: 'Latch', inputs: [{ name: 'u', type: 'Boolean' }, { name: 'clr', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'VAVBoxController', label: 'VAV template', inputs: [], outputs: [], parameters: [] },
]
