import assert from 'node:assert/strict'
import test from 'node:test'
import { orthogonalWire, routeWirePoints, wireBridges, type GraphBox } from '../src/lib/graphRouting'

function clearOf(box:GraphBox,a:{x:number;y:number},b:{x:number;y:number}):boolean {
  const left=box.x-12,right=box.x+box.width+12,top=box.y-12,bottom=box.y+box.height+12
  if(a.y===b.y)return !(a.y>top&&a.y<bottom&&Math.max(a.x,b.x)>left&&Math.min(a.x,b.x)<right)
  return !(a.x>left&&a.x<right&&Math.max(a.y,b.y)>top&&Math.min(a.y,b.y)<bottom)
}

test('routes around a block instead of crossing its face',()=>{
  const from={x:0,y:0},to={x:100,y:0},box={x:40,y:-10,width:20,height:20}
  const points=routeWirePoints(from,to,{obstacles:[box]})
  assert.deepEqual(points[0],from)
  assert.deepEqual(points.at(-1),to)
  for(let i=1;i<points.length;i++){
    assert.ok(points[i-1].x===points[i].x||points[i-1].y===points[i].y,'each segment stays orthogonal')
    assert.ok(clearOf(box,points[i-1],points[i]),'wire clears the block and its 12px margin')
  }
  assert.match(orthogonalWire(from,to,{obstacles:[box]}),/Q/,'bends have curved corners')
})

test('routes a backward connection above or below intervening blocks',()=>{
  const from={x:400,y:180},to={x:80,y:40},box={x:180,y:50,width:130,height:110}
  const points=routeWirePoints(from,to,{obstacles:[box]})
  assert.deepEqual(points[0],from)
  assert.deepEqual(points.at(-1),to)
  for(let i=1;i<points.length;i++)assert.ok(clearOf(box,points[i-1],points[i]))
})

test('bridges crossings of different signals but keeps shared-source branches joined',()=>{
  const vertical={source:'room_temp',points:[{x:40,y:0},{x:40,y:100}]}
  const horizontal={source:'supply_air_temp',points:[{x:0,y:50},{x:100,y:50}]}
  assert.deepEqual(wireBridges([vertical,horizontal]),[{x:40,y:50}])
  assert.deepEqual(wireBridges([vertical,{...horizontal,source:'room_temp'}]),[])
})
