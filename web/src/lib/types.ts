export interface Connector { name: string; type: 'Real' | 'Boolean' | 'Integer' | 'String'; unit?: string; description?: string; label?: string; brick_class?: string }
export interface Parameter { name: string; type?: string; value: number | boolean | string; unit?: string; description?: string }
export interface Equation { lhs: string; rhs: string }
export interface Connection { from?: string; to?: string; from_block?: string; from_output?: string; to_block?: string; to_input?: string }
export interface Block {
  name: string; block_type: string; category?: string; description?: string; notes?: string;
  inputs?: Connector[]; outputs?: Connector[]; parameters?: Parameter[]; equations?: Equation[];
  blocks?: Block[]; connections?: Connection[]
}
export interface Scenario { zone_ua_kw_per_k: number; zone_capacity_kwh_per_k: number; heat_pump_fraction: number; step_seconds: 15 | 30 | 60 | 300 | 900 }
export interface Project { version: 1; name: string; graph: Block; scenario: Scenario; layout: Record<string, { x: number; y: number }> }
export interface Row {
  index: number; outdoor_c: number; avg_zone_c: number; zone_c: number[];
  cool_kw: number; heat_kw: number; cool_request_kw: number; heat_request_kw: number;
  electric_kw: number; gas_kw: number; requested_airflow: number; delivered_airflow: number;
  ahu_1_supply_actual_c: number; ahu_2_supply_actual_c: number;
  ahu_1_supply_setpoint_c: number; ahu_2_supply_setpoint_c: number;
  heat_pump_heating_used_kw: number; heat_pump_cooling_used_kw: number;
  boiler_used_kw: number; chiller_used_kw: number; traces?: Record<string,number>
}
export interface Summary { electricity_kwh: number; gas_kwh: number; unmet_zone_hours: number }
export const defaultScenario: Scenario = { zone_ua_kw_per_k: 0.15, zone_capacity_kwh_per_k: 5, heat_pump_fraction: 0.55, step_seconds: 900 }

export function edgeEndpoints(edge: Connection): [string, string] {
  const source = edge.from ?? (edge.from_output ? `${edge.from_block}.${edge.from_output}` : edge.from_block ?? '')
  const target = edge.to ?? (edge.to_input ? `${edge.to_block}.${edge.to_input}` : edge.to_block ?? '')
  return [source, target]
}

export function layoutFor(graph: Block, legacy = false): Record<string, { x: number; y: number }> {
  const layout: Record<string, { x: number; y: number }> = {}
  function visit(block: Block, prefix: string) {
    const children=block.blocks??[]
    if(prefix==='' && !legacy && block.block_type==='BuildingController'){
      children.forEach((child,index)=>{
        const zone=Number(child.name.match(/^zone_(\d{2})$/)?.[1]??0)
        layout[child.name]=child.name==='plant'?{x:60,y:80}:child.name==='ahu_1'?{x:460,y:80}:child.name==='ahu_2'?{x:1260,y:80}:zone>0?{x:zone<=5?840:1640,y:80+((zone-1)%5)*240}:{x:60+(index%4)*400,y:80+Math.floor(index/4)*240}
      })
    }else if(prefix===''){
      children.forEach((child,index)=>{layout[`${prefix}${child.name}`]={x:60+(index%5)*(legacy?250:360),y:80+Math.floor(index/5)*(legacy?270:240)}})
    }else{
      const names=new Set(children.map(c=>c.name))
      const levels=new Map<string,number>()
      let pending=[...children]
      while(pending.length){
        const ready=pending.filter(child=>(block.connections??[]).every(edge=>{const [source,target]=edgeEndpoints(edge);return target.split('.')[0]!==child.name||!names.has(source.split('.')[0])||levels.has(source.split('.')[0])}))
        if(!ready.length)break
        for(const child of ready){const dependencies=(block.connections??[]).filter(edge=>edgeEndpoints(edge)[1].split('.')[0]===child.name).map(edge=>edgeEndpoints(edge)[0].split('.')[0]).filter(name=>names.has(name));levels.set(child.name,Math.max(0,...dependencies.map(name=>(levels.get(name)??-1)+1)))}
        pending=pending.filter(child=>!levels.has(child.name))
      }
      const rowByLevel=new Map<number,number>()
      children.forEach((child,index)=>{const level=levels.get(child.name)??index%5;const row=rowByLevel.get(level)??0;rowByLevel.set(level,row+1);layout[`${prefix}${child.name}`]={x:60+level*(legacy?250:360),y:80+row*(legacy?190:240)}})
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
  { type: 'CDL.Reals.Min', label: 'Min', inputs: [{ name: 'u1', type: 'Real' }, { name: 'u2', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [] },
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
  { type: 'VAVBoxController', label: 'VAV template', inputs: [], outputs: [], parameters: [] },
]
