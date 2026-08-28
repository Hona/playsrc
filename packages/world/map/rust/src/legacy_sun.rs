use crate::{legacy_glow::Quad,pixel_visibility::View};

pub fn colors(main:[u8;3],overlay:[u8;3])->[[f32;3];2]{
    let maximum=main.into_iter().max().unwrap();
    let main=if maximum==0{[1.0;3]}else{main.map(|v|f32::from(v)/f32::from(maximum))};
    let overlay=if overlay==[0;3]{main}else{overlay.map(|v|f32::from(v)/255.0)};
    [main,overlay]
}

pub fn quad(view:&View,direction:[f32;3],size:i32,color:[f32;3],fraction:f32,overlay:bool)->Option<Quad>{
    let length=(direction[0]*direction[0]+direction[1]*direction[1]+direction[2]*direction[2]).sqrt();
    let direction=if length>0.0{direction.map(|v|v/length)}else{direction};
    let dot=direction[0]*view.forward[0]+direction[1]*view.forward[1]+direction[2]*view.forward[2];
    let alpha=if overlay{(0.75+(0.0-0.75)*((dot-1.0)/(0.9-1.0))).clamp(0.0,0.75)}else{1.0};
    let size=size as f32*if overlay{6.0}else{1.0};
    crate::legacy_glow::overlay_quad(view,direction,[size,size],color.map(|v|v*alpha*fraction))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn sun_preserves_core_luminosity_and_authored_overlay_color(){assert_eq!(colors([100,50,25],[0,0,0]),[[1.0,0.5,0.25];2]);assert_eq!(colors([0;3],[255,0,0]),[[1.0;3],[1.0,0.0,0.0]]);}
    #[test]
    fn authored_direction_produces_an_in_front_screen_space_proxy(){
        let direction=[-0.6123724,-0.35355338,0.70710677];
        let view=View::perspective([100.0,200.0,768.0],-150.0,-45.0,59.840443,16.0/9.0,7.0,28377.92,720);
        let position=std::array::from_fn(|axis|view.origin[axis]+direction[axis]*(view.far*0.999));
        let proxy=view.proxy(crate::pixel_visibility::Parameters{position,size:0.05,aspect:1.0,screen_space:true}).unwrap();
        assert!(proxy.clip_fraction>0.99);
        assert!(proxy.clip_vertices.iter().all(|v|v[3]>0.0&&v[2]>=0.0&&v[2]<v[3]));
    }
}
