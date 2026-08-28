use std::{collections::BTreeMap,sync::Arc};
use playsrc_collision::{ObjectInput,ObjectRole,Snapshot,SnapshotShape,World};
use playsrc_entity::{EntityWorld,WorldCommand,spotlight::Seed};

/// Rebuild activation traces from live brushes after round cleanup, rather than
/// treating a previous round's endpoints as collision authority.
#[derive(Clone,Debug)]
pub(crate) struct Collision {world:Arc<World>,inputs:Arc<[ObjectInput]>}

impl Collision {
    pub fn new(world:Arc<World>,inputs:Vec<ObjectInput>)->Self {
        // StandardFilterRules admits static props, but rejects non-brush
        // entities when the mask does not contain CONTENTS_MONSTER.
        Self{world,inputs:inputs.into_iter().filter(|input|input.role==ObjectRole::StaticProp||matches!(input.shape,SnapshotShape::BrushModel{..})).collect()}
    }

    pub fn prepare(&self,entities:&EntityWorld)->Result<(Vec<Seed>,Vec<WorldCommand>),u32>{
        let live=entities.live_handles().into_iter().filter_map(|handle|entities.entity(handle).map(|entity|(entity.source_index as u64,entity))).collect::<BTreeMap<_,_>>();
        if !live.values().any(|entity|entity.classname.eq_ignore_ascii_case(b"point_spotlight")){return Ok((Vec::new(),Vec::new()));}
        let inputs=self.inputs.iter().cloned().map(|mut input|{
            if input.role==ObjectRole::Entity {
                if let Some(entity)=live.get(&input.identity){
                    let state=entities.collision_state(entity.handle).expect("live collision entity");
                    input.transform=playsrc_collision::Transform{origin:state.transform.origin,angles:state.transform.angles};input.enabled=state.enabled;
                }else{input.enabled=false;}
            }
            input
        }).collect();
        let snapshot=Snapshot::compile(&self.world,1,inputs,playsrc_collision::SnapshotLimits::default()).map_err(|_|0_u32)?;
        let mut scratch=playsrc_collision::QueryScratch::default();
        playsrc_entity::spotlight::prepare(entities,|_,_,start,end,ignore|{
            if ignore{return Ok(end);}
            self.world.trace_snapshot_hull_with_scratch(&snapshot,playsrc_collision::SnapshotTraceRequest{
                start,end,hull:playsrc_collision::Hull{mins:[0.0;3],maxs:[0.0;3]},mask:0x400b,
                scope:playsrc_collision::TraceScope::Everything,ignored:&[],
            },&mut scratch,|_|true).map(|trace|trace.end).map_err(|_|())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn object(identity:u64,role:ObjectRole,x:f32)->ObjectInput {
        ObjectInput{identity,role,enabled:true,volume_contents:false,transform:playsrc_collision::Transform{origin:[x,0.0,0.0],angles:[0.0;3]},linear_velocity:[0.0;3],angular_velocity:[0.0;3],collision_group:0,contents:1,surface_flags:0,shape:SnapshotShape::BoundingBox{bounds:playsrc_collision::Hull{mins:[-1.0;3],maxs:[1.0;3]}}}
    }
    fn entity_world(x:f32,ignore:bool)->EntityWorld {
        let graph=playsrc_entity::parse(format!("{{\"classname\"\"point_spotlight\"\"origin\"\"{x} 0 0\"\"spawnflags\"\"3\"\"SpotlightLength\"\"100\"\"SpotlightWidth\"\"10\"\"IgnoreSolid\"\"{}\"}}",u8::from(ignore)).as_bytes(),playsrc_entity::Limits::default()).unwrap();
        EntityWorld::compile(&graph,playsrc_entity::EntityWorldConfig{external_classes:playsrc_entity::spotlight::bindings().collect(),..Default::default()}).unwrap().0
    }
    fn activate(scene:&Collision,world:&mut EntityWorld)->playsrc_entity::spotlight::Beam {
        let (seeds,commands)=scene.prepare(world).unwrap();world.phase(0,&commands).unwrap();playsrc_entity::spotlight::bind(world,seeds).unwrap().remove(0)
    }
    #[test]
    fn brush_mask_keeps_static_props_but_not_nonbrush_entities_and_requeries_activation(){
        let scene=Collision::new(Arc::new(World::empty()),vec![object(10,ObjectRole::Entity,20.0),object(11,ObjectRole::StaticProp,40.0)]);
        assert_eq!(scene.inputs.len(),1);
        let beam=activate(&scene,&mut entity_world(0.0,false));assert!((beam.end[0]-39.0).abs()<0.1);
        let recreated=activate(&scene,&mut entity_world(100.0,false));assert_eq!(recreated.end,[200.0,0.0,0.0]);assert_eq!(recreated.end_width,20.0);
        let ignored=activate(&scene,&mut entity_world(0.0,true));assert_eq!(ignored.end,[100.0,0.0,0.0]);assert_eq!(ignored.end_width,20.0);
    }
}
