use std::{collections::BTreeMap,sync::Arc};
use crate::{Entity,EntityHandle,EntityWorld,ExternalClassBinding,Variant,source_integer};

pub const SIMULATE:u16=1<<3;
pub const INITIAL_HANG:u16=1<<6;
pub const NO_WIND:u16=1<<5;
#[derive(Clone,Debug,PartialEq)]
pub struct Definition {
    pub source:u32,pub entity:EntityHandle,pub end:Option<EntityHandle>,pub length:i32,pub slack:i32,
    pub nodes:u8,pub width:f32,pub texture_scale:f32,pub subdivisions:u8,pub flags:u16,pub locked:u8,
    pub material:Arc<str>,pub scroll_speed:f32,
}
#[derive(Clone,Debug,Default)]
pub struct Ropes(BTreeMap<u32,Definition>);

pub fn is_rope(class:&[u8])->bool{class.eq_ignore_ascii_case(b"move_rope")||class.eq_ignore_ascii_case(b"keyframe_rope")}
pub fn bindings()->Vec<ExternalClassBinding>{[b"move_rope".as_slice(),b"keyframe_rope"].into_iter().map(|classname|ExternalClassBinding{classname:classname.to_vec(),inputs:vec![b"SetScrollSpeed".to_vec()]}).collect()}

pub fn material(entity:&Entity)->Result<String,std::str::Utf8Error>{
    let mut result="cable/cable.vmt".to_owned();
    for pair in &entity.pairs{
        if pair.key.eq_ignore_ascii_case(b"RopeShader"){
            result=match source_integer(&pair.value){0=>"cable/cable.vmt",1=>"cable/rope.vmt",_=>"cable/chain.vmt"}.into();
        }else if pair.key.eq_ignore_ascii_case(b"RopeMaterial"){
            result=std::str::from_utf8(&pair.value)?.replace('\\',"/").to_ascii_lowercase();
            if !result.contains(".vmt"){result.push_str(".vmt");}
        }
    }
    let end=result.find(".vmt").expect("rope material extension");result.truncate(end);
    Ok(format!("materials/{result}.vmt"))
}

impl Ropes {
    pub fn from_world(world:&EntityWorld)->Result<Self,u32>{
        let mut output=Self::default();
        for entity in world.live_handles(){
            let state=world.entity(entity).expect("live rope");if !is_rope(&state.classname){continue;}
            let source=state.source_index as u32;
            let mut definition=Definition{source,entity,end:None,length:0,slack:0,nodes:5,width:2.0,texture_scale:4.0,subdivisions:2,flags:SIMULATE|INITIAL_HANG,locked:3,material:material(&state.definition).map_err(|_|source)?.into(),scroll_speed:0.0};
            if definition.material.split('/').any(|part|part.is_empty()||part==".."||part=="."||part==".vmt"){return Err(source);}
            let mut next=&b""[..];let mut resize=false;
            for pair in &state.definition.pairs{
                let integer=source_integer(&pair.value);
                let number=||Variant::String(pair.value.clone()).as_float().ok_or(source);
                let key=pair.key.as_slice();
                if key.eq_ignore_ascii_case(b"NextKey"){next=&pair.value;}
                else if key.eq_ignore_ascii_case(b"Slack"){definition.slack=integer;}
                else if key.eq_ignore_ascii_case(b"Width"){definition.width=number()?;}
                else if key.eq_ignore_ascii_case(b"TextureScale"){definition.texture_scale=number()?;}
                else if key.eq_ignore_ascii_case(b"Subdiv"){definition.subdivisions=integer as u8;}
                else if key.eq_ignore_ascii_case(b"Type"){definition.nodes=match integer{0=>10,1=>4,_=>2};}
                else if key.eq_ignore_ascii_case(b"Dangling")&&integer==1{definition.locked&=!2;}
                else if key.eq_ignore_ascii_case(b"Collide")&&integer==1{definition.flags|=1<<2;}
                else if key.eq_ignore_ascii_case(b"Barbed")&&integer==1{definition.flags|=1<<1;}
                else if key.eq_ignore_ascii_case(b"Breakable")&&integer==1{definition.flags|=1<<4;}
                else if key.eq_ignore_ascii_case(b"NoWind")&&integer==1{definition.flags|=NO_WIND;}
                else if key.eq_ignore_ascii_case(b"spawnflags"){resize=integer&1!=0;}
            }
            if !definition.width.is_finite()||!definition.texture_scale.is_finite()||definition.width<0.0||definition.texture_scale<=0.0{return Err(source);}
            definition.end=world.resolve(next,Some(entity),None,None).first().copied();
            if let Some(end)=definition.end.and_then(|handle|world.entity(handle)){
                definition.length=state.world_transform.origin.iter().zip(end.world_transform.origin).map(|(a,b)|(a-b)*(a-b)).sum::<f32>().sqrt() as i32;
                if resize{definition.flags|=1;}
            }else if definition.locked&2!=0{definition.flags&=!SIMULATE;}
            output.0.insert(source,definition);
        }
        Ok(output)
    }
    pub fn definitions(&self)->impl Iterator<Item=&Definition>{self.0.values()}
    pub fn get(&self,world:&EntityWorld,source:u32)->Option<(Definition,[Option<[f32;3]>;2])>{
        let definition=self.0.get(&source)?;let start=world.entity(definition.entity)?.world_transform.origin;
        let end=definition.end.and_then(|entity|world.entity(entity)).map(|entity|entity.world_transform.origin);
        Some((definition.clone(),[Some(start),end]))
    }
    pub fn reconcile(&mut self,world:&EntityWorld)->Result<(),u32>{
        let mut next=Self::from_world(world)?;
        for (source,definition) in &mut next.0{if let Some(prior)=self.0.get(source).filter(|prior|prior.entity==definition.entity){*definition=prior.clone();}}
        *self=next;Ok(())
    }
    pub fn input(&mut self,world:&EntityWorld,entity:EntityHandle,input:&[u8],value:&Variant)->Result<(),u32>{
        let source=world.entity(entity).map_or(u32::MAX,|entity|entity.source_index as u32);
        if let Some(rope)=self.0.get_mut(&source).filter(|rope|rope.entity==entity)&&input.eq_ignore_ascii_case(b"SetScrollSpeed"){
            let speed=value.as_float().filter(|value|value.is_finite()).ok_or(source)?;
            rope.scroll_speed=speed;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests{
    use super::*;use crate::{parse,Limits,EntityWorldConfig};
    #[test]
    fn authored_links_defaults_integer_length_and_legacy_material_keys_follow_source_order(){
        let graph=parse(br#"{"classname""move_rope""targetname""start""origin""0 0 0""NextKey""end""Slack""110""Type""1""RopeShader""2""RopeMaterial""cable/rope"}
            {"classname""keyframe_rope""targetname""end""origin""100.9 0 0"}
            {"classname""keyframe_rope""Dangling""1""NoWind""1""NoWind""0"}"#,Limits::default()).unwrap();
        let world=EntityWorld::compile(&graph,EntityWorldConfig{external_classes:bindings(),..Default::default()}).unwrap().0;
        let mut ropes=Ropes::from_world(&world).unwrap();let (first,positions)=ropes.get(&world,0).unwrap();
        assert_eq!(first.length,100);assert_eq!(first.slack,110);assert_eq!(first.nodes,4);assert_eq!(first.width,2.0);assert_eq!(first.texture_scale,4.0);
        assert_eq!(first.material.as_ref(),"materials/cable/rope.vmt");assert_eq!(positions[1],Some([100.9,0.0,0.0]));
        assert_eq!(ropes.get(&world,1).unwrap().0.flags&SIMULATE,0);
        let dangling=ropes.get(&world,2).unwrap().0;assert_eq!(dangling.locked,1);assert_ne!(dangling.flags&SIMULATE,0);assert_ne!(dangling.flags&NO_WIND,0);
        ropes.input(&world,first.entity,b"SetScrollSpeed",&Variant::Float(2.0_f32.to_bits())).unwrap();ropes.reconcile(&world).unwrap();assert_eq!(ropes.get(&world,0).unwrap().0.scroll_speed,2.0);
    }
}
