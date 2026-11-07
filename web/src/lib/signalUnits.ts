import { edgeEndpoints, type Block, type Connector } from './types'

const colors: Record<string,string> = {
  '°C':'#ff9d72', K:'#f2cb65', kW:'#ec83b6',
  'fraction (0–1)':'#c1d301', 'normalized airflow':'#6bd5e6',
  'kg/s':'#6bd5e6',
  'normalized pressure':'#ad9dff', Pa:'#ad9dff', 'm³/s':'#6bd5e6',
  Boolean:'#e8a8f4', unitless:'#aeb9ca', s:'#e6af79', '1/s':'#d6b38f',
}

export function normalizeSignalUnit(unit?:string,type?:string):string|undefined {
  if(type==='Boolean')return 'Boolean'
  if(!unit)return undefined
  const value=unit.trim()
  if(value==='degC'||value==='Cel'||value==='°C')return '°C'
  if(value==='m3/s'||value==='m³/s')return 'm³/s'
  if(value==='1'||value==='fraction (0–1)'||value==='fraction (0-1)')return 'fraction (0–1)'
  return value
}

export function signalColor(unit?:string):string { return colors[unit??'']??'#9eabbc' }

/** Infer display units for CDL ports whose source files omit unit metadata. */
export function resolveSignalUnits(graph:Block):Map<string,string> {
  const units=new Map<string,string>()
  const blocks=new Map((graph.blocks??[]).map(block=>[block.name,block]))
  const seed=(key:string,port:Connector)=>{const unit=normalizeSignalUnit(port.unit,port.type);if(unit)units.set(key,unit)}
  for(const port of graph.inputs??[])seed(port.name,port)
  for(const port of graph.outputs??[])seed(port.name,port)
  for(const block of graph.blocks??[]){
    for(const port of block.inputs??[])seed(`${block.name}.${port.name}`,port)
    for(const port of block.outputs??[])seed(`${block.name}.${port.name}`,port)
    if(block.block_type.split('.').at(-1)==='Constant'){
      const unit=normalizeSignalUnit(block.parameters?.find(parameter=>parameter.name==='k')?.unit)
      if(unit)units.set(`${block.name}.y`,unit)
    }
  }
  const set=(key:string,unit?:string)=>{if(!unit||units.has(key))return false;units.set(key,unit);return true}
  const edges=(graph.connections??[]).map(edge=>edgeEndpoints(edge))
  const isZero=(key:string)=>{const [name,port]=key.split('.');const block=blocks.get(name);return port==='y'&&block?.block_type.split('.').at(-1)==='Constant'&&block.parameters?.find(parameter=>parameter.name==='k')?.value===0}
  for(const [from,to] of edges)if(!isZero(from))set(to,units.get(from))
  // Work backward from declared graph outputs first. This preserves conversions
  // such as K → fraction across gain blocks whose own ports lack unit metadata.
  for(let pass=0;pass<Math.max(4,blocks.size*2);pass++){
    let changed=false
    for(const [from,to] of edges)if(!isZero(from))changed=set(from,units.get(to))||changed
    for(const block of blocks.values()){
      const kind=block.block_type.split('.').at(-1)??''
      const key=(port:string)=>`${block.name}.${port}`
      const output=units.get(key('y'))
      if(output&&['Limiter','Switch','Add','Min','Max'].includes(kind)){
        for(const port of block.inputs??[])if(!(kind==='Switch'&&port.name==='u3'))changed=set(key(port.name),output)||changed
      }
      if(output&&kind==='Multiply'){
        const first=units.get(key('u1')),second=units.get(key('u2'))
        if(first==='fraction (0–1)'||first==='normalized pressure')changed=set(key('u2'),output)||changed
        if(second==='fraction (0–1)'||second==='normalized pressure')changed=set(key('u1'),output)||changed
      }
      if(kind==='Switch')changed=set(key('u3'),'Boolean')||changed
    }
    if(!changed)break
  }
  // Then spread input units through arithmetic and comparison blocks.
  for(let pass=0;pass<Math.max(4,blocks.size*2);pass++){
    let changed=false
    for(const [from,to] of edges){if(!isZero(from)){changed=set(to,units.get(from))||changed;changed=set(from,units.get(to))||changed}}
    for(const block of blocks.values()){
      const kind=block.block_type.split('.').at(-1)??''
      const key=(port:string)=>`${block.name}.${port}`
      const input=(port:string)=>units.get(key(port))
      const first=input('u1'),second=input('u2')
      if(['Subtract','Add','Greater','Less','Min','Max'].includes(kind)){
        changed=set(key('u1'),second)||changed
        changed=set(key('u2'),first)||changed
      }
      const data=(block.inputs??[]).filter(port=>port.name!=='u3'||kind!=='Switch').map(port=>input(port.name)).filter((unit):unit is string=>!!unit&&unit!=='Boolean')
      let inferred:string|undefined
      if(['Greater','Less','And','Or','Not'].includes(kind))inferred='Boolean'
      else if(kind==='Subtract')inferred=first&&second?(first==='°C'&&second==='°C'?'K':first===second?first:undefined):undefined
      else if(kind==='Multiply')inferred=data.find(unit=>unit!=='fraction (0–1)'&&unit!=='normalized pressure')??data[0]
      else if(kind==='LimPID')inferred='fraction (0–1)'
      else if(kind!=='Constant')inferred=data[0]
      changed=set(key('y'),inferred)||changed
      if(kind==='Switch')changed=set(key('u3'),'Boolean')||changed
    }
    if(!changed)break
  }
  return units
}
