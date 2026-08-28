use std::collections::BTreeMap;
use crate::{EntityHandle,EntityWorld,ExternalClassBinding,Variant,source_integer};

#[derive(Clone,Debug,PartialEq)]
pub struct Presentation {pub source:u32,pub entity:EntityHandle,pub direction:[f32;3],pub size:i32,pub overlay_size:i32,pub overlay_color:[u8;4],pub hdr_scale:f32,pub active:bool}

#[derive(Clone,Debug,Default)]
pub struct Suns(BTreeMap<u32,Presentation>);

pub fn binding()->ExternalClassBinding {ExternalClassBinding{classname:b"env_sun".to_vec(),inputs:[b"TurnOn".as_slice(),b"TurnOff",b"SetColor"].into_iter().map(<[u8]>::to_vec).collect()}}

impl Suns {
    pub fn from_world(world:&EntityWorld)->Result<Self,u32> {
        let mut result=Self::default();
        for entity in world.live_handles(){
            let state=world.entity(entity).expect("live sun");if !state.classname.eq_ignore_ascii_case(b"env_sun"){continue;}
            let source=state.source_index as u32;
            let value=|key:&[u8]|state.definition.pairs.iter().find(|pair|pair.key.eq_ignore_ascii_case(key)).map(|pair|pair.value.as_slice());
            let integer=|key,default|value(key).map_or(default,source_integer);
            let number=|key,default|value(key).map(|value|Variant::String(value.to_vec()).as_float().ok_or(source)).unwrap_or(Ok(default));
            let mut direction=[0.0,0.0,1.0];
            if integer(b"use_angles",0)!=0 {
                let mut yaw=number(b"angle",0.0)?;let mut pitch=number(b"pitch",0.0)?;
                if yaw == -1.0 {direction=[0.0,0.0,1.0];}
                else if yaw == -2.0 {direction=[0.0,0.0,-1.0];}
                else {if yaw==0.0{yaw=state.world_transform.angles[1];}let yaw=(yaw/180.0) as f64*std::f64::consts::PI;direction=[yaw.cos() as f32,yaw.sin() as f32,0.0];}
                if pitch==0.0{pitch=state.world_transform.angles[0];}
                let pitch=(pitch/180.0) as f64*std::f64::consts::PI;
                direction=[-direction[0]*pitch.cos() as f32,-direction[1]*pitch.cos() as f32,-(pitch.sin() as f32)];
            }else if let Some(target)=value(b"target").and_then(|name|world.resolve(name,Some(entity),None,None).first().copied()).and_then(|target|world.entity(target)) {
                direction=std::array::from_fn(|axis|state.world_transform.origin[axis]-target.world_transform.origin[axis]);
                let length=(direction[0]*direction[0]+direction[1]*direction[1]+direction[2]*direction[2]).sqrt();if length>0.0{direction=direction.map(|v|v/length);}
            }
            let size=integer(b"size",16);let overlay=integer(b"overlaysize",-1);
            result.0.insert(source,Presentation{source,entity,direction,size,overlay_size:if overlay == -1 {size}else{overlay},overlay_color:value(b"overlaycolor").map_or([0;4],crate::value::source_color),hdr_scale:number(b"HDRColorScale",0.0)?,active:true});
        }
        Ok(result)
    }
    pub fn get(&self,world:&EntityWorld,source:u32)->Option<Presentation>{let sun=self.0.get(&source)?;world.entity(sun.entity).map(|_|sun.clone())}
    pub fn reconcile(&mut self,world:&EntityWorld)->Result<(),u32>{
        let mut next=Self::from_world(world)?;
        for (source,sun) in &mut next.0 {if let Some(old)=self.0.get(source).filter(|old|old.entity==sun.entity){*sun=old.clone();}}
        *self=next;Ok(())
    }
    pub fn input(&mut self,world:&EntityWorld,entity:EntityHandle,input:&[u8],value:&Variant)->Option<Variant>{
        let source=world.entity(entity)?.source_index as u32;let sun=self.0.get_mut(&source)?;
        if sun.entity!=entity{return None;}
        if input.eq_ignore_ascii_case(b"TurnOn"){sun.active=true;}else if input.eq_ignore_ascii_case(b"TurnOff"){sun.active=false;}
        else if input.eq_ignore_ascii_case(b"SetColor"){return Some(value.clone());}
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;use crate::{parse,Limits,EntityWorldConfig};
    #[test]
    fn sun_defaults_angles_targets_and_inputs_are_server_owned(){
        let graph=parse(br#"{"classname" "env_sun" "use_angles" "1" "pitch" "-45" "angle" "90"}
          {"classname" "env_sun" "target" "aim" "origin" "10 0 0" "size" "20"}
          {"classname" "info_target" "targetname" "aim" "origin" "0 0 0"}"#,Limits::default()).unwrap();
        let world=EntityWorld::compile(&graph,EntityWorldConfig{external_classes:vec![binding()],..Default::default()}).unwrap().0;
        let mut suns=Suns::from_world(&world).unwrap();let first=suns.get(&world,0).unwrap();
        assert!((first.direction[1]+std::f32::consts::FRAC_1_SQRT_2).abs()<1e-6);assert!((first.direction[2]-std::f32::consts::FRAC_1_SQRT_2).abs()<1e-6);
        let second=suns.get(&world,1).unwrap();assert_eq!(second.direction,[1.0,0.0,0.0]);assert_eq!(second.overlay_size,20);assert!(second.active);
        suns.input(&world,second.entity,b"TurnOff",&Variant::Void);assert!(!suns.get(&world,1).unwrap().active);
        suns.reconcile(&world).unwrap();assert!(!suns.get(&world,1).unwrap().active);
    }
}
