export interface GraphPoint { x: number; y: number }
export interface GraphBox { x: number; y: number; width: number; height: number }
export interface WireOptions { obstacles?: GraphBox[]; radius?: number }

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

function score(points:GraphPoint[],obstacles:GraphBox[]):number {
  let total=0
  for(let i=1;i<points.length;i++){
    const a=points[i-1],b=points[i]
    total+=Math.abs(a.x-b.x)+Math.abs(a.y-b.y)
    for(const box of obstacles)if(intersects(a,b,box))total+=100000
  }
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

/** Route through clear lanes around node boxes, then round each right-angle bend. */
export function routeWirePoints(from:GraphPoint,to:GraphPoint,options:WireOptions={}):GraphPoint[] {
  const obstacles=options.obstacles??[]
  const paths:GraphPoint[][]=[]
  const add=(points:GraphPoint[])=>paths.push(simplify(points))
  if(Math.abs(from.y-to.y)<.5)add([from,to])
  const middle=(from.x+to.x)/2
  const xLanes=[middle,from.x+32,to.x-32]
  for(const box of obstacles)xLanes.push(box.x-24,box.x+box.width+24)
  for(const x of xLanes){
    if(x<=from.x+4||x>=to.x-4)continue
    add([from,point(x,from.y),point(x,to.y),to])
  }
  const yLanes=[(from.y+to.y)/2,Math.min(from.y,to.y)-36,Math.max(from.y,to.y)+36]
  for(const box of obstacles)yLanes.push(box.y-24,box.y+box.height+24)
  for(const sourceX of [from.x+20,from.x+32])for(const targetX of [to.x-20,to.x-32])for(const y of yLanes)add([from,point(sourceX,from.y),point(sourceX,y),point(targetX,y),point(targetX,to.y),to])
  if(!paths.length)add([from,point(middle,from.y),point(middle,to.y),to])
  let best=paths[0],bestScore=score(best,obstacles)
  for(const candidate of paths.slice(1)){const value=score(candidate,obstacles);if(value<bestScore){best=candidate;bestScore=value}}
  return best
}

export function orthogonalWire(from:GraphPoint,to:GraphPoint,options:WireOptions={}):string {
  return rounded(routeWirePoints(from,to,options),options.radius??10)
}
