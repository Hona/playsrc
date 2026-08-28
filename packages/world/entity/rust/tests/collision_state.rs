use playsrc_entity::*;

#[test]
fn parented_prop_collision_tracks_world_transform_and_collision_inputs(){
    let graph=parse(br#"{"classname""info_target""targetname""parent""origin""100 200 300"}
        {"classname""prop_dynamic""targetname""door_prop""model""models/door.mdl""parentname""parent""origin""110 210 320""solid""6"}"#,Limits::default()).unwrap();
    let mut world=EntityWorld::compile(&graph,EntityWorldConfig::default()).unwrap().0;
    let parent=world.resolve(b"parent",None,None,None)[0];let child=world.resolve(b"door_prop",None,None,None)[0];
    assert_eq!(world.collision_state(child).unwrap(),EntityCollisionState{transform:Transform{origin:[110.0,210.0,320.0],angles:[0.0;3]},enabled:true});
    world.phase(0,&[WorldCommand::SetWorldTransform{entity:parent,transform:Transform{origin:[100.0,200.0,364.0],angles:[0.0;3]}}]).unwrap();
    assert_eq!(world.collision_state(child).unwrap().transform.origin,[110.0,210.0,384.0]);
    for (tick,input,enabled) in [(1,b"DisableCollision".as_slice(),false),(2,b"EnableCollision".as_slice(),true)]{
        world.phase(tick,&[WorldCommand::Input(InputRecord{target:EventTarget::Expression(b"door_prop".to_vec()),input:input.to_vec(),value:Variant::Void,activator:None,caller:None,output_action:None,producer_sequence:0})]).unwrap();
        assert_eq!(world.collision_state(child).unwrap().enabled,enabled);
    }
    world.phase(3,&[WorldCommand::Remove(child)]).unwrap();assert!(world.collision_state(child).is_none());
}

#[test]
fn map_recreation_links_forward_parents_before_activation_and_preserves_existing_entities(){
    let graph=parse(br#"{"classname""prop_dynamic""targetname""child""model""models/door.mdl""parentname""parent""origin""10 0 0""solid""6"}
        {"classname""info_target""targetname""parent""origin""0 0 0"}"#,Limits::default()).unwrap();
    let mut world=EntityWorld::compile(&graph,EntityWorldConfig::default()).unwrap().0;
    let original=world.resolve(b"child",None,None,None)[0];let parent=world.resolve(b"parent",None,None,None)[0];
    world.phase(0,&[WorldCommand::Remove(original)]).unwrap();
    world.phase(1,&[WorldCommand::SpawnMapEntities{definitions:graph.entities.clone(),excluded_sources:vec![1]}]).unwrap();
    let child=world.resolve(b"child",None,None,None)[0];assert_ne!(child,original);assert_eq!(world.entity(child).unwrap().parent,Some(parent));
    world.phase(2,&[WorldCommand::SetWorldTransform{entity:parent,transform:Transform{origin:[0.0,0.0,128.0],angles:[0.0;3]}}]).unwrap();
    assert_eq!(world.collision_state(child).unwrap().transform.origin,[10.0,0.0,128.0]);
}
