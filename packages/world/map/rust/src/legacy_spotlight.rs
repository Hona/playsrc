use crate::pixel_visibility::View;

#[derive(Clone,Debug)]
pub struct Quad {pub positions:[[f32;3];4],pub colors:[[f32;4];4],pub uv:[[f32;2];4]}
pub struct Geometry {pub beam:Quad,pub halo:Option<Quad>,pub halo_proxy_size:f32}

pub fn geometry(view:&View,start:[f32;3],end:[f32;3],width:f32,end_width:f32,color:[u8;4],halo_fraction:f32)->Option<Geometry>{
    let delta=sub(end,start);let beam_length=length(delta);if beam_length<0.1{return None;}
    let direction=scale(delta,1.0/beam_length);let local=sub(view.origin,start);let unit=normalize(local);
    let facing=dot(direction,unit);let fade=if facing<0.0{0.0}else{facing*2.0};
    let on_line=add(start,scale(direction,dot(local,direction)));let distance=length(sub(view.origin,on_line));
    let threshold=width*4.0;
    let dot_scale=if distance<threshold {remap(distance,threshold,width,1.0,0.0).clamp(0.0,1.0)}else{1.0};
    let base=std::array::from_fn::<_,3,_>(|axis|f32::from(color[axis])/255.0);
    let beam_color=packed([base[0]*f32::from(color[3])/255.0*dot_scale,base[1]*f32::from(color[3])/255.0*dot_scale,base[2]*f32::from(color[3])/255.0*dot_scale]);
    let normal=normalize(cross(sub(start,end),sub(start,view.origin)));let side=scale(normal,width);
    let beam=Quad{positions:[add(start,side),sub(start,side),sub(end,side),add(end,side)],colors:[beam_color,beam_color,[0.0,0.0,0.0,1.0],[0.0,0.0,0.0,1.0]],uv:[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]};
    let halo=if fade!=0.0&&halo_fraction>0.0{
        let size=remap(distance,threshold,width*0.5,1.0,2.0).clamp(1.0,2.0)*60.0;
        let amount=(fade*fade).clamp(0.0,1.0)*halo_fraction;
        let color=packed([base[0]*amount,base[1]*amount,base[2]*amount]);
        let right=scale(view.right,size);let up=scale(view.up,size);
        Some(Quad{positions:[sub(sub(start,up),right),sub(add(start,up),right),add(add(start,up),right),add(sub(start,up),right)],colors:[color;4],uv:[[0.0,1.0],[0.0,0.0],[1.0,0.0],[1.0,1.0]]})
    }else{None};
    Some(Geometry{beam,halo,halo_proxy_size:(1.0+60.0*width/end_width).clamp(1.0,8.0)})
}
fn packed(rgb:[f32;3])->[f32;4]{let rgb=rgb.map(|v|((v*255.0+8_388_608.0).to_bits()&255) as f32/255.0);[rgb[0],rgb[1],rgb[2],1.0]}
fn remap(v:f32,a:f32,b:f32,c:f32,d:f32)->f32{if a==b {if v>=b {d}else{c}}else{c+(d-c)*((v-a)/(b-a))}}
fn add(a:[f32;3],b:[f32;3])->[f32;3]{std::array::from_fn(|i|a[i]+b[i])}
fn sub(a:[f32;3],b:[f32;3])->[f32;3]{std::array::from_fn(|i|a[i]-b[i])}
fn scale(a:[f32;3],b:f32)->[f32;3]{a.map(|v|v*b)}
fn dot(a:[f32;3],b:[f32;3])->f32{a[0]*b[0]+a[1]*b[1]+a[2]*b[2]}
fn cross(a:[f32;3],b:[f32;3])->[f32;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn length(a:[f32;3])->f32{dot(a,a).sqrt()}
fn normalize(a:[f32;3])->[f32;3]{let n=length(a);if n==0.0{a}else{scale(a,1.0/n)}}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn efficient_spotlight_has_constant_width_shadeout_and_front_only_halo(){
        let view=View::perspective([100.0,100.0,0.0],180.0,0.0,75.0,1.0,1.0,30000.0,720);
        let value=geometry(&view,[0.0;3],[200.0,0.0,0.0],10.0,20.0,[255,128,64,64],1.0).unwrap();
        assert_eq!(value.halo_proxy_size,8.0);assert!(value.halo.is_some());
        assert_eq!(value.beam.colors[2],[0.0,0.0,0.0,1.0]);assert_eq!(value.beam.colors[0],[64.0/255.0,32.0/255.0,16.0/255.0,1.0]);
        let behind=View{origin:[-100.0,100.0,0.0],..view};assert!(geometry(&behind,[0.0;3],[200.0,0.0,0.0],10.0,20.0,[255;4],1.0).unwrap().halo.is_none());
    }
}
