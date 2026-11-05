export interface GraphPoint { x: number; y: number }
export interface GraphBox { x: number; y: number; width: number; height: number }
export interface RoutedTrack { source:string; points:GraphPoint[] }
export interface WireOptions { obstacles?: GraphBox[]; radius?: number; targetLane?: number; tracks?: RoutedTrack[]; source?: string }

const point = (x:number,y:number):GraphPoint => ({x,y})
const same = (a:GraphPoint,b:GraphPoint) => Math.abs(a.x-b.x)<.5 && Math.abs(a.y-b.y)<.5

function simplify(points:GraphPoint[]):GraphPoint[] {
  const result:GraphPoint[]=[]
  for(const p of points){
    if(result.length&&same(result[result.length-1],p))continue
    const last=result[result.length-1],before=result[result.length-2]
    if(before&&last&&((Math.abs(before.x-last.x)<.5&&Math.abs(last.x-p.x)<.5)||(Math.abs(before.y-last.y)<.5&&Math.abs(last.y-p.y)<.5)))result.pop()
    result.push(p)
  }
  return result
}

function intersects(a:GraphPoint,b:GraphPoint,box:GraphBox):boolean {
  const left=box.x-12,right=box.x+box.width+12,top=box.y-12,bottom=box.y+box.height+12
  if(Math.abs(a.y-b.y)<.5)return a.y>top&&a.y<bottom&&Math.max(a.x,b.x)>left&&Math.min(a.x,b.x)<right
  return a.x>left&&a.x<right&&Math.max(a.y,b.y)>top&&Math.min(a.y,b.y)<bottom
}

function score(points:GraphPoint[],obstacles:GraphBox[],targetLane?:number,tracks:RoutedTrack[]=[],source?:string):number {
  let total=0
  for(let i=1;i<points.length;i++){
    const a=points[i-1],b=points[i]
    total+=Math.abs(a.x-b.x)+Math.abs(a.y-b.y)
    for(const box of obstacles)if(intersects(a,b,box))total+=100000
    for(const track of tracks){
      if(track.source===source)continue
      for(let j=1;j<track.points.length;j++){
        const c=track.points[j-1],d=track.points[j]
        const aVertical=Math.abs(a.x-b.x)<.5,cVertical=Math.abs(c.x-d.x)<.5
        if(aVertical===cVertical){
          const distance=aVertical?Math.abs(a.x-c.x):Math.abs(a.y-c.y)
          const overlap=aVertical?Math.min(Math.max(a.y,b.y),Math.max(c.y,d.y))-Math.max(Math.min(a.y,b.y),Math.min(c.y,d.y)):Math.min(Math.max(a.x,b.x),Math.max(c.x,d.x))-Math.max(Math.min(a.x,b.x),Math.min(c.x,d.x))
          if(distance<16&&overlap>0)total+=overlap*(16-distance)*.8
        }else{
          const vertical=aVertical?[a,b]:[c,d],horizontal=aVertical?[c,d]:[a,b]
          const x=vertical[0].x,y=horizontal[0].y
          if(x>Math.min(horizontal[0].x,horizontal[1].x)+6&&x<Math.max(horizontal[0].x,horizontal[1].x)-6&&y>Math.min(vertical[0].y,vertical[1].y)+6&&y<Math.max(vertical[0].y,vertical[1].y)-6)total+=90
        }
      }
    }
  }
  if(targetLane!==undefined&&points.length>2)total+=Math.abs(points[points.length-2].x-targetLane)*2
  return total+(points.length-2)*22
}

function rounded(points:GraphPoint[],radius:number):string {
  if(points.length<2)return ''
  let path=`M${points[0].x} ${points[0].y}`
  for(let i=1;i<points.length-1;i++){
    const prev=points[i-1],current=points[i],next=points[i+1]
    const before=Math.abs(current.x-prev.x)+Math.abs(current.y-prev.y)
    const after=Math.abs(next.x-current.x)+Math.abs(next.y-current.y)
    const bend=Math.min(radius,before/2,after/2)
    const inX=Math.sign(current.x-prev.x),inY=Math.sign(current.y-prev.y)
    const outX=Math.sign(next.x-current.x),outY=Math.sign(next.y-current.y)
    if(!bend||inX===outX&&inY===outY){path+=` L${current.x} ${current.y}`;continue}
    path+=` L${current.x-inX*bend} ${current.y-inY*bend} Q${current.x} ${current.y} ${current.x+outX*bend} ${current.y+outY*bend}`
  }
  const last=points[points.length-1]
  return `${path} L${last.x} ${last.y}`
}

export function roundedWire(points:GraphPoint[],radius=10):string { return rounded(points,radius) }

/** Crossings between distinct signals get a bridge; branches of one signal remain joined. */
export function wireBridges(wires:Array<{source:string;points:GraphPoint[]}>):GraphPoint[] {
  const result:GraphPoint[]=[],seen=new Set<string>()
  for(let i=0;i<wires.length;i++)for(let j=i+1;j<wires.length;j++){
    if(wires[i].source===wires[j].source)continue
    const first=wires[i].points,second=wires[j].points
    for(let a=1;a<first.length;a++)for(let b=1;b<second.length;b++){
      const p=first[a-1],q=first[a],r=second[b-1],s=second[b]
      const vertical=Math.abs(p.x-q.x)<.5,otherVertical=Math.abs(r.x-s.x)<.5
      if(vertical===otherVertical)continue
      const v1=vertical?p:r,v2=vertical?q:s,h1=vertical?r:p,h2=vertical?s:q
      const x=v1.x,y=h1.y
      if(x<=Math.min(h1.x,h2.x)+10||x>=Math.max(h1.x,h2.x)-10)continue
      if(y<=Math.min(v1.y,v2.y)+10||y>=Math.max(v1.y,v2.y)-10)continue
      const key=`${Math.round(x)}:${Math.round(y)}`
      if(!seen.has(key)){seen.add(key);result.push({x,y})}
    }
  }
  return result
}

/** Route through clear lanes around node boxes, then round each right-angle bend. */
export function routeWirePoints(from:GraphPoint,to:GraphPoint,options:WireOptions={}):GraphPoint[] {
  const obstacles=options.obstacles??[]
  const minX=Math.min(from.x,to.x)-80,maxX=Math.max(from.x,to.x)+80
  const minY=Math.min(from.y,to.y)-80,maxY=Math.max(from.y,to.y)+80
  const nearby=obstacles.filter(box=>box.x<=maxX&&box.x+box.width>=minX&&box.y<=maxY&&box.y+box.height>=minY)
  const middleX=(from.x+to.x)/2,middleY=(from.y+to.y)/2
  const laneObstacles=nearby.length>16?[...nearby].sort((a,b)=>Math.abs(a.x+a.width/2-middleX)+Math.abs(a.y+a.height/2-middleY)-Math.abs(b.x+b.width/2-middleX)-Math.abs(b.y+b.height/2-middleY)).slice(0,16):nearby
  const paths:GraphPoint[][]=[]
  const add=(points:GraphPoint[])=>paths.push(simplify(points))
  if(Math.abs(from.y-to.y)<.5)add([from,to])
  const middle=(from.x+to.x)/2
  const xLanes=[middle,middle-24,middle+24,middle-48,middle+48,from.x+32,to.x-32,...(options.targetLane===undefined?[]:[options.targetLane])]
  for(const box of laneObstacles)xLanes.push(box.x-24,box.x+box.width+24)
  for(const x of xLanes){
    if(x<=from.x+4||x>=to.x-4)continue
    add([from,point(x,from.y),point(x,to.y),to])
  }
  const yLanes=[(from.y+to.y)/2,Math.min(from.y,to.y)-36,Math.max(from.y,to.y)+36]
  for(const box of laneObstacles)yLanes.push(box.y-24,box.y+box.height+24)
  const targetLanes=[to.x-20,to.x-32,...(options.targetLane===undefined?[]:[options.targetLane])]
  for(const sourceX of [from.x+20,from.x+32])for(const targetX of targetLanes)for(const y of yLanes)add([from,point(sourceX,from.y),point(sourceX,y),point(targetX,y),point(targetX,to.y),to])
  if(!paths.length)add([from,point(middle,from.y),point(middle,to.y),to])
  let best=paths[0],bestScore=score(best,obstacles,options.targetLane,options.tracks,options.source)
  for(const candidate of paths.slice(1)){const value=score(candidate,obstacles,options.targetLane,options.tracks,options.source);if(value<bestScore){best=candidate;bestScore=value}}
  return best
}

export function orthogonalWire(from:GraphPoint,to:GraphPoint,options:WireOptions={}):string {
  return roundedWire(routeWirePoints(from,to,options),options.radius??10)
}
