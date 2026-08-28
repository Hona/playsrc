//! Shared camera-facing beam strips for Source rope renderers.
pub type Vector=[f32;3];
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Segment {pub position:Vector,pub width:f32,pub color:[f32;4],pub texture_coordinate:f32}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Vertex {pub position:Vector,pub uv:[f32;2],pub color:[u8;4]}
#[derive(Clone,Debug,PartialEq)]
pub struct Mesh {pub vertices:Vec<Vertex>,pub indices:Vec<u32>}

pub struct Builder {mesh:Mesh,camera:Vector,previous:Option<Segment>,normal:Option<Vector>}
impl Builder {
    pub fn new(points:usize,camera:Vector)->Self{
        assert!(points>=2&&points<=u32::MAX as usize/2);
        Self{mesh:Mesh{vertices:Vec::with_capacity(points*2),indices:Vec::with_capacity((points-1)*6)},camera,previous:None,normal:None}
    }
    pub fn push(&mut self,next:Segment){
        if let Some(previous)=self.previous{
            let normal=normalize(cross(sub(previous.position,next.position),sub(previous.position,self.camera)));
            let averaged=self.normal.map_or(normal,|prior|normalize(scale(add(normal,prior),0.5)));
            self.emit(previous,averaged);self.normal=Some(normal);
        }
        self.previous=Some(next);
    }
    fn emit(&mut self,segment:Segment,normal:Vector){
        let mesh=&mut self.mesh;
        let side=scale(normal,segment.width*0.5);
        let color=segment.color.map(|value|((value*255.0+8_388_608.0).to_bits()&255) as u8);
        mesh.vertices.push(Vertex{position:add(segment.position,side),uv:[0.0,segment.texture_coordinate],color});
        mesh.vertices.push(Vertex{position:sub(segment.position,side),uv:[1.0,segment.texture_coordinate],color});
        if mesh.vertices.len()>2{let base=mesh.vertices.len() as u32-4;mesh.indices.extend_from_slice(&[base,base+1,base+2,base+1,base+3,base+2]);}
    }
    pub fn finish(mut self)->Mesh{self.emit(self.previous.expect("beam segment"),self.normal.expect("two beam segments"));self.mesh}
}
pub fn build(segments:&[Segment],camera:Vector)->Mesh{
    let mut builder=Builder::new(segments.len(),camera);for &segment in segments{builder.push(segment);}builder.finish()
}
fn add(a:Vector,b:Vector)->Vector{std::array::from_fn(|i|a[i]+b[i])}
fn sub(a:Vector,b:Vector)->Vector{std::array::from_fn(|i|a[i]-b[i])}
fn scale(a:Vector,b:f32)->Vector{a.map(|v|v*b)}
fn normalize(a:Vector)->Vector{let n=(a[0]*a[0]+a[1]*a[1]+a[2]*a[2]+1.0e-10).sqrt();scale(a,n.recip())}
fn cross(a:Vector,b:Vector)->Vector{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn shared_strip_keeps_source_segment_width_uv_color_and_winding(){
        let mesh=build(&[Segment{position:[0.0;3],width:2.0,color:[1.0,0.0,0.0,1.0],texture_coordinate:0.0},Segment{position:[10.0,0.0,0.0],width:4.0,color:[0.0,1.0,0.0,0.5],texture_coordinate:2.0}],[0.0,0.0,10.0]);
        assert_eq!(mesh.vertices[0].position,[0.0,-1.0,0.0]);assert_eq!(mesh.vertices[3].position,[10.0,2.0,0.0]);
        assert_eq!(mesh.vertices[3].color,[0,255,0,128]);assert_eq!(mesh.vertices[3].uv,[1.0,2.0]);assert_eq!(mesh.indices,[0,1,2,1,3,2]);
    }
    #[test]
    fn vertex_bytes_use_source_even_rounding_at_half_byte_boundaries(){
        let a=Segment{position:[0.0;3],width:1.0,color:[0.5/255.0,1.5/255.0,2.5/255.0,1.0],texture_coordinate:0.0};
        let mesh=build(&[a,Segment{position:[1.0,0.0,0.0],..a}],[0.0,0.0,10.0]);
        assert_eq!(mesh.vertices[0].color,[0,2,2,255]);
    }
}
