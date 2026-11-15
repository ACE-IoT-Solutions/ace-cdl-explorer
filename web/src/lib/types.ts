export interface Connector { name: string; type: 'Real' | 'Boolean' | 'Integer' | 'String'; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string; label?: string; brick_class?: string }
export interface Parameter { name: string; type?: string; value: number | boolean | string; unit?: string; cxf_unit?: string; cxf_display_unit?: string; description?: string }
export interface Equation { lhs: string; rhs: string }
export interface Connection { from?: string; to?: string; from_block?: string; from_output?: string; to_block?: string; to_input?: string }
export interface BrickEntity { id: string; class: string; label: string; controller?: string; connector?: string }
export interface BrickRelationship { subject: string; predicate: 'brick:hasPart' | 'brick:feeds' | 'brick:hasPoint'; object: string }
export interface BrickModel { entities: BrickEntity[]; relationships: BrickRelationship[] }
export interface Block {
  name: string; block_type: string; category?: string; description?: string; notes?: string;
  inputs?: Connector[]; outputs?: Connector[]; parameters?: Parameter[]; equations?: Equation[];
  blocks?: Block[]; connections?: Connection[]
  brick_model?: BrickModel
}
export type * from './generated'
import type { Scenario as NativeScenario, SimulationRow, SimulationTotals } from './generated'
import { measureColumns, zoneRowColumns } from './generated/rowColumns'
// This retired setting belongs to project-v1 migrations, not the native simulator.
export type Scenario = NativeScenario & { heat_pump_fraction?: number }
export type FanFault = import('./generated').EquipmentOutage
export interface Project { version: 1; name: string; graph: Block; scenario: Scenario; layout: Record<string, { x: number; y: number }> }
// Arrow publishes scalar measurements and zone arrays; JSON-only diagnostics and
// selected traces remain optional in this transport view of the native row.
export type Row = Pick<SimulationRow, 'index' | typeof measureColumns[number] | typeof zoneRowColumns[number]> & Partial<SimulationRow>
export type Summary = SimulationTotals
export const defaultScenario: Scenario = { zone_ua_kw_per_k: 0.15, zone_capacity_kwh_per_k: 5, terminal_design_flow: Array(10).fill(0.95), terminal_reheat_valve_stroke_seconds: Array(10).fill(0), ahu_heating_coil_valve_stroke_seconds: [0,0], ahu_cooling_coil_valve_stroke_seconds: [0,0], ahu_fan_design_airflow: [5,5], ahu_fan_startup_seconds: [0,0], ahu_damper_stroke_seconds: 300, water_pump_design_flow_kg_s: [4.31,4.31], water_volume_kg: [5000,5000], water_pump_startup_seconds: [0,0], water_pump_shutoff_pressure_pa: [80000,80000], water_bypass_conductance: [0.2,0.2], water_coil_conductance: [0.4,0.8], hot_reheat_conductance: 0.4, zone_air_volume_m3: 250, zone_infiltration_m3_s: 0.01, open_window_additional_infiltration_m3_s: 0.2, design_occupants_per_zone: 8, co2_generation_lps_per_person: 0.004, occupant_sensible_heat_kw_per_person: 0.075, occupant_moisture_kg_h_per_person: 0.06, outdoor_co2_ppm: 420, outdoor_relative_humidity: null, return_air_relative_humidity: null, pollution_operator: 0, pollution_on_duration_s: 3600, local_utc_offset_hours: 0, clock_offset_events: [], air_quality_events: [], outdoor_pm25_ug_m3: null, outdoor_ozone_ppm: null, outdoor_aqi: null, heat_pump_capacity_kw: 80, heat_recovery_compressor_savings_fraction: 1, heat_pump_performance: [{outdoor_c:-15,capacity_fraction:0.55,heating_cop:2,cooling_cop:3},{outdoor_c:10,capacity_fraction:1,heating_cop:3.2,cooling_cop:3.8},{outdoor_c:35,capacity_fraction:0.8,heating_cop:2.7,cooling_cop:3}], heat_pump_part_load_performance: [{load_fraction:0,cop_multiplier:1},{load_fraction:1,cop_multiplier:1}], heat_pump_hot_water_cop_performance: [{water_c:35,cop_multiplier:1,capacity_multiplier:1},{water_c:55,cop_multiplier:1,capacity_multiplier:1}], heat_pump_chilled_water_cop_performance: [{water_c:4,cop_multiplier:1,capacity_multiplier:1},{water_c:17,cop_multiplier:1,capacity_multiplier:1}], heat_pump_joint_performance: null, equipment_outages: [], equipment_start_failures: [], fan_faults: [], fan_speed_sensor_events: [], water_pump_faults: [], water_flow_sensor_events: [], terminal_faults: [], terminal_valve_sensor_events: [], ahu_damper_faults: [], ahu_coil_valve_faults: [], ahu_damper_sensor_events: [], ahu_coil_valve_sensor_events: [], ahu_mixed_air_sensor_events: [], outdoor_temperature_events: [], window_openings: [], occupancy_overrides: [], occupancy_sensor_events: [], occupant_count_events: [], occupancy_count_sensor_events: [], alarm_acknowledgements: [], ahu_alarm_acknowledgements: [], plant_alarm_acknowledgements: [], equipment_startup_seconds: {heat_pump:0,boiler:0,chiller:0}, heat_pump_fault_reset_hour: -1, boiler_fault_reset_hour: -1, chiller_fault_reset_hour: -1, boiler_capacity_kw: 100, chiller_capacity_kw: 100, step_seconds: 900, freeze_test_ahu: 0, freeze_test_start_hour: -1, freeze_test_duration_hours: 0, freeze_test_reset_hour: -1 }

export function edgeEndpoints(edge: Connection): [string, string] {
  const source = edge.from ?? (edge.from_output ? `${edge.from_block}.${edge.from_output}` : edge.from_block ?? '')
  const target = edge.to ?? (edge.to_input ? `${edge.to_block}.${edge.to_input}` : edge.to_block ?? '')
  return [source, target]
}

export function layoutFor(graph: Block, legacy: boolean | 'previous' | 'depth24' = false): Record<string, { x: number; y: number }> {
  const layout: Record<string, { x: number; y: number }> = {}
  function visit(block: Block, prefix: string) {
    const children=block.blocks??[]
    const isStateBreak=(child:Block)=>['UnitDelay','Pre'].includes(child.block_type.split('.').at(-1)??'')
    if(prefix==='' && legacy!==true && block.block_type==='BuildingController'){
      const zoneRows=new Map<number,number>()
      if(legacy===false){
        for(const first of [1,6]){
          let y=80
          for(let zone=first;zone<first+5;zone++){
            zoneRows.set(zone,y)
            const child=children.find(item=>item.name===`zone_${String(zone).padStart(2,'0')}`)
            const rows=Math.max(child?.inputs?.length??0,child?.outputs?.length??0)
            const height=Math.max(140,68+rows*28+30+(child?.equations?.length?38:0))
            y+=Math.max(280,height+40)
          }
        }
      }
      children.forEach((child,index)=>{
        const zone=Number(child.name.match(/^zone_(\d{2})$/)?.[1]??0)
        const flowOverrideZone=Number(child.name.match(/^zone_(\d{2})_flow_override_none$/)?.[1]??0)
        if(legacy==='previous'){
          layout[child.name]=child.name==='plant'?{x:60,y:80}
            :child.name==='ahu_1'?{x:460,y:80}
            :child.name==='ahu_2'?{x:1260,y:80}
            :zone>0?{x:zone<=5?840:1640,y:80+((zone-1)%5)*240}
            :{x:60+(index%4)*400,y:80+Math.floor(index/4)*240}
          return
        }
        const spacing=legacy==='depth24'?240:280
        if(child.name==='plant')layout[child.name]={x:60,y:80}
        else if(legacy===false&&child.name==='plant_alarm_acknowledgement'){
          const source=children.find(item=>item.name==='plant')
          const rows=Math.max(source?.inputs?.length??0,source?.outputs?.length??0)
          const height=Math.max(140,68+rows*28+30+(source?.equations?.length?38:0))
          layout[child.name]={x:60,y:80+height+80}
        }
        else if(child.name==='ahu_1')layout[child.name]={x:860,y:80}
        else if(child.name==='ahu_2')layout[child.name]={x:2460,y:80}
        else if(legacy===false&&/^ahu_[12]_equipment_alarm_acknowledgement$/.test(child.name)){
          const ahu=child.name.startsWith('ahu_1_')?1:2
          const source=children.find(item=>item.name===`ahu_${ahu}`)
          const rows=Math.max(source?.inputs?.length??0,source?.outputs?.length??0)
          const height=Math.max(140,68+rows*28+30+(source?.equations?.length?38:0))
          layout[child.name]={x:ahu===1?860:2460,y:80+height+80}
        }
        else if(flowOverrideZone>0&&legacy===false){
          const sourceZone=children.find(item=>item.name===`zone_${String(flowOverrideZone).padStart(2,'0')}`)
          const inputIndex=sourceZone?.inputs?.findIndex(port=>port.name==='flow_override_code')??0
          layout[child.name]={x:flowOverrideZone<=5?1260:2860,y:(zoneRows.get(flowOverrideZone)??80)+Math.max(0,inputIndex)*28}
        }else if(legacy===false&&/^ahu_[12]_alarm_acknowledgement$/.test(child.name)){
          const last=child.name.startsWith('ahu_1_')?5:10
          const lastZone=children.find(item=>item.name===`zone_${String(last).padStart(2,'0')}`)
          const rows=Math.max(lastZone?.inputs?.length??0,lastZone?.outputs?.length??0)
          const height=Math.max(140,68+rows*28+30+(lastZone?.equations?.length?38:0))
          layout[child.name]={x:last===5?1660:3260,y:(zoneRows.get(last)??80)+height+80}
        }else if(zone>0)layout[child.name]={x:zone<=5?1660:3260,y:legacy===false?(zoneRows.get(zone)??80):80+((zone-1)%5)*spacing}
        else layout[child.name]={x:60+(index%4)*800,y:80+Math.floor(index/4)*spacing}
      })
    }else if(legacy===false||legacy==='depth24'){
      const names=new Set(children.map(child=>child.name))
      const byName=new Map(children.map(child=>[child.name,child]))
      const dependencies=new Map(children.map(child=>[child.name,isStateBreak(child)?[]:(block.connections??[]).filter(edge=>edgeEndpoints(edge)[1].split('.')[0]===child.name).map(edge=>edgeEndpoints(edge)[0].split('.')[0]).filter(name=>names.has(name))]))
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
        const ready=pending.filter(child=>isStateBreak(child)||(block.connections??[]).every(edge=>{const [source,target]=edgeEndpoints(edge);return target.split('.')[0]!==child.name||!names.has(source.split('.')[0])||levels.has(source.split('.')[0])}))
        if(!ready.length)break
        for(const child of ready){const dependencies=isStateBreak(child)?[]:(block.connections??[]).filter(edge=>edgeEndpoints(edge)[1].split('.')[0]===child.name).map(edge=>edgeEndpoints(edge)[0].split('.')[0]).filter(name=>names.has(name));levels.set(child.name,Math.max(0,...dependencies.map(name=>(levels.get(name)??-1)+1)))}
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

export { palette } from './blockCatalog'
