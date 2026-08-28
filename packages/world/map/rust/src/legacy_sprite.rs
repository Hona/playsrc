//! Source sprite model axes, mapping-pixel extents and glow dissipation.
use crate::pixel_visibility::View;
use playsrc_material::legacy_sprite::Orientation;

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Quad { pub positions:[[f32;3];4], pub uv:[[f32;2];4], pub color:[u8;4] }

pub fn quad(view:&View,position:[f32;3],angles:[f32;3],orientation:Orientation,extents:[f32;4],size:[u32;2],
    mut scale_value:f32,mode:u8,fx:u8,mut color:[u8;4],render_blend:u8,visibility:f32,fov_distance_adjust:f32)->Option<Quad> {
    if mode==6||mode==10||render_blend==0 {return None;}
    if mode==3||mode==9 {
        let distance=length(sub(position,view.origin))*fov_distance_adjust;
        if visibility<=0.0||distance<=0.0 {return None;}
        let blend=if fx==14 {color[3] as f32*(1.0/255.0)*visibility}
            else {
                if mode!=9 {if scale_value==0.0 {scale_value=1.0;}scale_value*=distance*(1.0/200.0);}
                ((1200.0*1200.0)/(distance*distance)).clamp(0.0,1.0)*visibility
            };
        let blend=blend*(render_blend as f32/255.0);
        for component in &mut color[..3] {*component=(*component as f32*blend) as u8;}
    }
    let (right,up)=axes(view,position,angles,orientation)?;
    let scale_value=if scale_value>0.0 {scale_value} else {1.0};
    let [left,right_extent,top,bottom]=extents.map(|value|value*scale_value);
    let a=add(position,scale(up,bottom));let c=add(position,scale(up,top));
    let b=scale(right,left);let d=scale(right,right_extent);
    let min_u=0.5/size[0] as f32;let min_v=0.5/size[1] as f32;
    Some(Quad{positions:[add(a,b),add(c,b),add(c,d),add(a,d)],uv:[[min_u,1.0-min_v],[min_u,min_v],[1.0-min_u,min_v],[1.0-min_u,1.0-min_v]],color})
}

fn axes(view:&View,origin:[f32;3],angles:[f32;3],mut orientation:Orientation)->Option<([f32;3],[f32;3])> {
    if orientation==Orientation::Parallel&&angles[2]!=0.0 {orientation=Orientation::ParallelOriented;}
    let upright=|direction:[f32;3]| {
        if direction[2]>0.999848||direction[2] < -0.999848 {return None;}
        Some((normalize([direction[1],-direction[0],0.0]),[0.0,0.0,1.0]))
    };
    match orientation {
        Orientation::Parallel=>Some((view.right,view.up)),
        // Source's facing-upright path uses the negated model origin directly.
        Orientation::FacingUpright=>upright(normalize(scale(origin,-1.0))),
        Orientation::ParallelUpright=>upright(view.forward),
        Orientation::ParallelOriented=>{
            let (s,c)=angles[2].to_radians().sin_cos();
            Some((add(scale(view.right,c),scale(view.up,s)),add(scale(view.right,-s),scale(view.up,c))))
        }
        Orientation::Oriented=>{
            let (sp,cp)=angles[0].to_radians().sin_cos();let (sy,cy)=angles[1].to_radians().sin_cos();let (sr,cr)=angles[2].to_radians().sin_cos();
            Some(([-sr*sp*cy+cr*sy,-sr*sp*sy-cr*cy,-sr*cp],[cr*sp*cy+sr*sy,cr*sp*sy-sr*cy,cr*cp]))
        }
    }
}
fn add(a:[f32;3],b:[f32;3])->[f32;3] {std::array::from_fn(|i|a[i]+b[i])}
fn sub(a:[f32;3],b:[f32;3])->[f32;3] {std::array::from_fn(|i|a[i]-b[i])}
fn scale(a:[f32;3],b:f32)->[f32;3] {a.map(|value|value*b)}
fn length(a:[f32;3])->f32 {(a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt()}
fn normalize(a:[f32;3])->[f32;3] {let n=length(a);if n==0.0 {a} else {scale(a,1.0/n)}}

#[cfg(test)]
mod tests {
    use super::*;
    fn view()->View {View::perspective([0.0;3],0.0,0.0,75.0,16.0/9.0,1.0,30000.0,720)}
    #[test]
    fn world_glow_and_no_dissipation_keep_authored_scale_and_integer_tint() {
        let draw=|mode,fx|quad(&view(),[400.0,0.0,0.0],[0.0;3],Orientation::Parallel,[-8.0,8.0,4.0,-4.0],[16,8],1.0,mode,fx,[255,128,64,128],128,0.5,1.0).unwrap();
        assert_eq!(draw(9,0).positions[0],[400.0,8.0,-4.0]);
        assert_eq!(draw(3,0).positions[0],[400.0,16.0,-8.0]);
        assert_eq!(draw(3,14).positions[0],[400.0,8.0,-4.0]);
        assert_eq!(draw(3,14).color,[32,16,8,128]);
        assert_eq!(draw(9,0).uv[0],[0.03125,0.9375]);
    }
    #[test]
    fn parallel_roll_and_upright_singularity_use_source_axes() {
        let view=view();let (right,up)=axes(&view,[100.0,0.0,0.0],[0.0,0.0,90.0],Orientation::Parallel).unwrap();
        assert!((right[2]-1.0).abs()<1e-6);assert!((up[1]-1.0).abs()<1e-6);
        let down=View::perspective([0.0;3],0.0,90.0,75.0,1.0,1.0,30000.0,720);
        assert!(axes(&down,[100.0,0.0,0.0],[0.0;3],Orientation::ParallelUpright).is_none());
    }
}
