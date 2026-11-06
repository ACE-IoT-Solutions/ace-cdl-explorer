export interface Connector { name: string; type: 'Real' | 'Boolean' | 'Integer' | 'String'; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string; label?: string; brick_class?: string }
export interface Parameter { name: string; type?: string; value: number | boolean | string; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string }
export interface Equation { lhs: string; rhs: string }
export interface Connection { from?: string; to?: string; from_block?: string; from_output?: string; to_block?: string; to_input?: string }
export interface Block {
  name: string; block_type: string; category?: string; description?: string; notes?: string;
  inputs?: Connector[]; outputs?: Connector[]; parameters?: Parameter[]; equations?: Equation[];
  blocks?: Block[]; connections?: Connection[]
}
export interface Scenario { zone_ua_kw_per_k: number; zone_capacity_kwh_per_k: number; heat_pump_fraction: number; heat_pump_capacity_kw: number; boiler_capacity_kw: number; chiller_capacity_kw: number; step_seconds: 15 | 30 | 60 | 300 | 900 }
export interface Project { version: 1; name: string; graph: Block; scenario: Scenario; layout: Record<string, { x: number; y: number }> }
export interface Row {
  index: number; outdoor_c: number; avg_zone_c: number; zone_c: number[];
  schedule_occupied?: number; preconditioning_mode?: number; zone_occupied?: number[]; zone_presence_raw?: number[];
  zone_airflow?: number[]; zone_discharge_c?: number[];
  cool_kw: number; heat_kw: number; cool_request_kw: number; heat_request_kw: number;
  electric_kw: number; gas_kw: number; requested_airflow: number; delivered_airflow: number;
  ahu_1_supply_actual_c: number; ahu_2_supply_actual_c: number;
  ahu_1_supply_setpoint_c: number; ahu_2_supply_setpoint_c: number;
  ahu_1_duct_pressure: number; ahu_2_duct_pressure: number;
  ahu_1_outdoor_air_fraction: number; ahu_2_outdoor_air_fraction: number;
  ahu_1_outdoor_airflow: number; ahu_2_outdoor_airflow: number;
  ahu_1_mixed_air_c: number; ahu_2_mixed_air_c: number;
  heat_pump_heating_used_kw: number; heat_pump_cooling_used_kw: number;
  boiler_used_kw: number; chiller_used_kw: number; traces?: Record<string,number>
}
export interface Summary { electricity_kwh: number; gas_kwh: number; unmet_zone_hours: number; occupied_unmet_zone_hours: number }
export const defaultScenario: Scenario = { zone_ua_kw_per_k: 0.15, zone_capacity_kwh_per_k: 5, heat_pump_fraction: 0.55, heat_pump_capacity_kw: 80, boiler_capacity_kw: 100, chiller_capacity_kw: 100, step_seconds: 900 }

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
  { type: 'Buildings.Controls.OBC.CDL.Logical.Latch', label: 'Latch', inputs: [{ name: 'u', type: 'Boolean' }, { name: 'clr', type: 'Boolean' }], outputs: [{ name: 'y', type: 'Boolean' }], parameters: [] },
  { type: 'VAVBoxController', label: 'VAV template', inputs: [], outputs: [], parameters: [] },
]
