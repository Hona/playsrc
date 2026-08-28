//! Efficient point_spotlight activation: the anonymous controller disappears,
//! while its generated, independently removable beam remains in the world.
use std::collections::BTreeMap;
use crate::{EntityHandle,EntityWorld,ExternalClassBinding,WorldCommand,Variant,parse,Limits,source_integer};

#[derive(Clone,Debug)]
pub struct Beam {pub source:u32,pub entity:EntityHandle,pub start:[f32;3],pub end:[f32;3],pub width:f32,pub end_width:f32,pub fade_length:f32,pub hdr_scale:f32,pub minimum_dx_level:i32}
#[derive(Clone,Debug)]
pub struct Seed {source:u32,generated:usize,start:[f32;3],end:[f32;3],width:f32,end_width:f32,fade_length:f32,hdr_scale:f32,minimum_dx_level:i32}

pub fn bindings()->impl Iterator<Item=ExternalClassBinding>{[
    ExternalClassBinding{classname:b"point_spotlight".to_vec(),inputs:vec![b"LightOn".to_vec(),b"LightOff".to_vec()]},
    ExternalClassBinding{classname:b"beam".to_vec(),inputs:vec![b"Width".to_vec()]},
].into_iter()}

pub fn prepare(world:&EntityWorld,mut trace:impl FnMut(u32,[f32;3],[f32;3],bool)->Result<[f32;3],()>)->Result<(Vec<Seed>,Vec<WorldCommand>),u32>{
    let mut seeds=Vec::new();let mut commands=Vec::new();
    for handle in world.live_handles(){
        let entity=world.entity(handle).expect("live spotlight");if !entity.classname.eq_ignore_ascii_case(b"point_spotlight"){continue;}
        let source=entity.source_index as u32;
        let field=|key:&[u8]|entity.definition.pairs.iter().find(|pair|pair.key.eq_ignore_ascii_case(key)).map(|pair|pair.value.as_slice()).unwrap_or_default();
        let integer=|key|source_integer(field(key));let number=|key|Variant::String(field(key).to_vec()).as_float().ok_or(source);
        // These require the moving endpoint/dynamic-light controller, rather than
        // Source's efficient static beam path used by the configured maps.
        if entity.parent.is_some()||entity.targetname.is_some()||integer(b"spawnflags")&2==0{return Err(source);}
        if integer(b"spawnflags")&1!=0{
            let length=number(b"SpotlightLength")?;let length=if length<=0.0{500.0}else{length};
            let width=number(b"SpotlightWidth")?;let width=if width<=0.0{10.0}else{width.min(102.3)};
            let angles=entity.world_transform.angles;let (sp,cp)=angles[0].to_radians().sin_cos();let (sy,cy)=angles[1].to_radians().sin_cos();let direction=[cp*cy,cp*sy,-sp];
            let start=entity.world_transform.origin;let destination=std::array::from_fn(|axis|start[axis]+direction[axis]*2.0*length);
            let end=trace(source,start,destination,integer(b"IgnoreSolid")!=0).map_err(|_|source)?;
            let distance=end.iter().zip(start).map(|(a,b)|(a-b)*(a-b)).sum::<f32>().sqrt();
            let generated=0x6000_0000usize+entity.source_index;
            let color=entity.render.color;
            let mut definition=parse(format!("{{\"classname\"\"beam\"\"model\"\"sprites/glow_test02.vmt\"\"origin\"\"{} {} {}\"\"rendermode\"\"2\"\"renderamt\"\"64\"\"rendercolor\"\"{} {} {}\"}}",start[0],start[1],start[2],color[0],color[1],color[2]).as_bytes(),Limits::default()).map_err(|_|source)?.entities.remove(0);
            definition.index=generated;commands.push(WorldCommand::Spawn(definition));
            commands.push(WorldCommand::EmitOutput{entity:handle,output:b"OnLightOn".to_vec(),value:Variant::Void,activator:Some(handle),caller:Some(handle),delay:0.0});
            let hdr_scale=if field(b"HDRColorScale").is_empty(){1.0}else{number(b"HDRColorScale")?};
            seeds.push(Seed{source,generated,start,end,width,end_width:(width*(distance/length)).clamp(0.0,102.3),fade_length:distance.min(length),hdr_scale,minimum_dx_level:integer(b"mindxlevel")});
        }
        commands.push(WorldCommand::Remove(handle));
    }
    Ok((seeds,commands))
}

pub fn bind(world:&EntityWorld,seeds:Vec<Seed>)->Result<Vec<Beam>,u32>{
    let handles:BTreeMap<_,_>=world.live_handles().into_iter().filter_map(|handle|world.entity(handle).map(|entity|(entity.source_index,handle))).collect();
    seeds.into_iter().map(|seed|Ok(Beam{source:seed.source,entity:*handles.get(&seed.generated).ok_or(seed.source)?,start:seed.start,end:seed.end,width:seed.width,end_width:seed.end_width,fade_length:seed.fade_length,hdr_scale:seed.hdr_scale,minimum_dx_level:seed.minimum_dx_level})).collect()
}

pub fn presentation(world:&EntityWorld,beam:&Beam)->Option<(Beam,crate::EntityRenderState)>{
    let entity=world.entity(beam.entity)?;let mut result=beam.clone();
    let local=crate::Transform{origin:std::array::from_fn(|axis|beam.end[axis]-beam.start[axis]),angles:[0.0;3]};
    result.start=entity.world_transform.origin;result.end=crate::world::compose_transform(entity.world_transform,local).origin;
    Some((result,entity.render.clone()))
}

#[cfg(test)]
mod tests{
    use super::*;use crate::{EntityWorldConfig,EventTarget,InputRecord};
    #[test]
    fn efficient_controller_removes_itself_but_its_beam_remains_an_entity(){
        let graph=parse(br#"{"classname" "point_spotlight" "origin" "0 0 10" "angles" "0 0 0" "spawnflags" "3" "SpotlightLength" "100" "SpotlightWidth" "10"}"#,Limits::default()).unwrap();
        let mut world=EntityWorld::compile(&graph,EntityWorldConfig{external_classes:bindings().collect(),..Default::default()}).unwrap().0;
        let (seeds,commands)=prepare(&world,|_,start,end,ignore|{assert!(!ignore);assert_eq!(start,[0.0,0.0,10.0]);assert_eq!(end,[200.0,0.0,10.0]);Ok([50.0,0.0,10.0])}).unwrap();
        world.phase(0,&commands).unwrap();let beams=bind(&world,seeds).unwrap();assert_eq!(beams.len(),1);assert_eq!(beams[0].end_width,5.0);
        assert!(world.resolve(b"point_spotlight",None,None,None).is_empty());assert_eq!(world.resolve(b"beam",None,None,None),vec![beams[0].entity]);
        world.phase(1,&[WorldCommand::Input(InputRecord{target:EventTarget::Expression(b"beam".to_vec()),input:b"Kill".to_vec(),value:Variant::Void,activator:None,caller:None,output_action:None,producer_sequence:0})]).unwrap();assert!(world.entity(beams[0].entity).is_none());
    }
}
