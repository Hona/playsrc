//! C_BaseEntity render-alpha effects, evaluated once per client render frame.
#[derive(Clone,Debug,Default)]
pub struct FxBlend {frame:Option<u32>,server_color:Option<[u8;4]>,alpha:u8,blend:u8}
impl FxBlend {
    pub fn sample(&mut self,frame:u32,index:u32,time:f32,mode:u8,fx:u8,color:[u8;4],view_distance:f32,fade:u8,mut random:impl FnMut(i32,i32)->i32)->u8 {
        if self.server_color!=Some(color) {self.server_color=Some(color);self.alpha=color[3];}
        if self.frame==Some(frame) {return self.blend;}
        self.frame=Some(frame);
        let offset=index as f32*363.0;
        let wave=|rate:f32,phase:f32|((time*rate+phase) as f64).sin();
        let pulse=|rate,amplitude|self.alpha as f64+amplitude*wave(rate,offset);
        let mut blend=match fx {
            1=>pulse(2.0,16.0) as i32,2=>pulse(8.0,16.0) as i32,
            3=>pulse(2.0,64.0) as i32,4=>pulse(8.0,64.0) as i32,
            5=>{self.alpha=self.alpha.saturating_sub(1);i32::from(self.alpha)},
            6=>{self.alpha=self.alpha.saturating_sub(4);i32::from(self.alpha)},
            7=>{self.alpha=self.alpha.saturating_add(1);i32::from(self.alpha)},
            8=>{self.alpha=self.alpha.saturating_add(4);i32::from(self.alpha)},
            9|10|11=>{let rate=match fx {9=>4.0,10=>16.0,_=>36.0};if ((20.0*wave(rate,offset)) as i32)<0 {0} else {i32::from(self.alpha)}},
            12|13=>{let (first,second)=if fx==12 {(2.0,17.0)}else{(16.0,23.0)};if ((20.0*(wave(first,0.0)+wave(second,offset))) as i32)<0 {0}else{i32::from(self.alpha)}},
            15|16=>{
                let distance=if fx==15 {1.0}else{view_distance};
                if distance<=0.0 {0}else{
                    self.alpha=180;
                    let base=if distance<=100.0 {180}else{((1.0-(distance as f64-100.0)*(1.0/400.0))*180.0) as i32};
                    base+random(-32,31)
                }
            }
            24=>(255.0*wave(12.0,offset).abs()) as i32,
            _=>if mode==0 {255}else{i32::from(self.alpha)},
        }.clamp(0,255);
        if fade!=255 {blend=((blend as f32/255.0)*(f32::from(fade)/255.0)*255.0+0.5) as i32;}
        self.blend=blend.clamp(0,255) as u8;self.blend
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fading_is_frame_counted_and_a_repeated_view_does_not_advance_it() {
        let mut fx=FxBlend::default();let color=[255;4];
        assert_eq!(fx.sample(1,1,0.0,5,6,color,0.0,255,|_,_|0),251);
        assert_eq!(fx.sample(1,1,0.5,5,6,color,0.0,255,|_,_|0),251);
        assert_eq!(fx.sample(2,1,0.5,5,6,color,0.0,255,|_,_|0),247);
        assert_eq!(fx.sample(3,1,0.5,5,6,[254,255,255,255],0.0,255,|_,_|0),251);
    }
    #[test]
    fn hologram_changes_client_alpha_and_no_dissipation_uses_normal_entity_alpha() {
        let mut fx=FxBlend::default();
        assert_eq!(fx.sample(1,1,0.0,9,14,[255,255,255,128],0.0,255,|_,_|panic!("no randomness")),128);
        assert_eq!(fx.sample(2,1,0.0,9,16,[255,255,255,128],300.0,255,|min,max|{assert_eq!((min,max),(-32,31));0}),90);
        assert_eq!(fx.sample(3,1,0.0,9,14,[255,255,255,128],0.0,255,|_,_|0),180);
    }
}
