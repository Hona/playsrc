use std::{collections::BTreeMap,sync::Arc};
use playsrc_map::{legacy_glow::LightGlow, pixel_visibility::{Query, View, DEFAULT_FADE_TIME}};

pub const GLOW_MATERIAL: &str = "materials/sprites/light_glow02_add_noz.vmt";

pub fn encode_materials(out: &mut Vec<u8>, graph: &playsrc_entity::Graph, bundle: &BTreeMap<String, &[u8]>,
    decoders: &super::TextureDecoders<'_>, hashes: &BTreeMap<String,[u8;32]>, profile: playsrc_map::LightingProfile) -> Result<(), ()> {
    let layout=super::legacy_materials::compile(graph,bundle,decoders,profile)?;
    out.extend_from_slice(b"PLVM"); out.extend_from_slice(&3_u32.to_le_bytes()); out.extend_from_slice(&(layout.materials.len() as u32).to_le_bytes());
    for asset in &layout.materials {
        let texture=super::model_authored_texture(&asset.texture,decoders,hashes,true)?;
        super::pbytes(out,asset.identity.as_bytes())?;
        super::encode_model_authored_texture(out,&asset.texture,&texture)?;
        let p=&asset.program;
        let flags=u32::from(p.srgb)|(u32::from(p.vertex_rgb)<<1)|(u32::from(p.vertex_alpha)<<2)|(u32::from(p.vertex_gamma)<<3)|(u32::from(p.gamma_exposure)<<4)|(u32::from(p.world_renderable)<<5)|(u32::from(p.cable)<<6);
        out.extend_from_slice(&flags.to_le_bytes());for value in p.modulation {out.extend_from_slice(&value.to_le_bytes());}
        if let Some(normal)=&asset.normal{let texture=super::model_authored_texture(normal,decoders,hashes,true).inspect_err(|_|eprintln!("Rope normal texture: {normal}"))?;super::encode_model_authored_texture(out,normal,&texture)?;}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rope_entity_removal_recreation_and_speculative_frames_preserve_client_ownership(){
        let mut layout=super::super::legacy_materials::Layout::default();layout.ropes.push(super::super::legacy_materials::RopeDefinition{source:7,material:0,back:None,mapping_height:16});
        let state=|generation|Some((playsrc_entity::rope::Definition{source:7,entity:playsrc_entity::EntityHandle{slot:7,generation},end:None,length:200,slack:120,nodes:5,width:1.0,texture_scale:1.0,subdivisions:2,flags:playsrc_entity::rope::SIMULATE|playsrc_entity::rope::INITIAL_HANG|playsrc_entity::rope::NO_WIND,locked:3,material:"materials/cable/cable.vmt".into(),scroll_speed:0.0},[Some([0.0;3]),Some([200.0,0.0,0.0]) ]));
        let mut world=World{layout:Arc::new(layout),ropes:vec![None],..Default::default()};world.initialize_ropes(|_|state(1),|_|Ok([0.5;3])).unwrap();
        let original=world.ropes[0].as_ref().unwrap().physics.nodes.clone();
        let runtime=Runtime::new(world);let (_,mut abandoned)=runtime.begin(1,0).unwrap();
        abandoned.advance_ropes(0.1,[0.0;3],|_|None,|_|panic!("removed rope must not sample lighting")).unwrap();assert!(abandoned.ropes[0].is_none());
        let (_,mut retry)=runtime.begin(1,0).unwrap();assert_eq!(retry.ropes[0].as_ref().unwrap().physics.nodes,original);
        let mut samples=0;retry.advance_ropes(0.0,[0.0;3],|_|state(2),|_|{samples+=1;Ok([0.25;3])}).unwrap();
        assert_eq!(samples,5);let rope=retry.ropes[0].as_ref().unwrap();assert_eq!(rope.definition.entity.generation,2);assert_eq!(rope.lighting[0],[0.25;3]);assert_eq!(rope.physics.nodes,original);
    }
    #[test]
    fn sun_client_frame_issues_both_native_proxy_channels(){
        let mut layout=super::super::legacy_materials::Layout::default();
        layout.suns.push(super::super::legacy_materials::SunDefinition{source:816,materials:[0,0]});
        let mut world=World{layout:Arc::new(layout),suns:vec![SunClient::default()],..Default::default()};
        let entity=playsrc_entity::EntityHandle{slot:816,generation:1};
        let sun=playsrc_entity::sun::Presentation{source:816,entity,direction:[-0.6123724,-0.35355338,0.70710677],size:18,overlay_size:15,overlay_color:[254,226,188,255],hdr_scale:1.0,active:true};
        let view=View::perspective([4455.78,-1042.0,931.75],-150.0,-45.0,59.840443,16.0/9.0,7.0,28377.92,720);
        let mut bytes=Vec::new();
        world.frame(&view,0,12.0,1,0.016,1.0,&[],|_|Some((entity,playsrc_entity::Transform::IDENTITY,playsrc_entity::EntityRenderState{brush_model:None,mode:0,color:[254,221,177,255],fx:0,effects:0},None)),|_|Some(sun.clone()),|_|None,|_,_|(true,true),&mut bytes).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()),2);
        assert_eq!(u32::from_le_bytes(bytes[20..24].try_into().unwrap()),0x8000_0000|816);
    }
    #[test]
    fn abandoned_render_candidates_do_not_advance_the_source_random_stream() {
        let graph=playsrc_entity::parse(br#"{"classname" "env_lightglow" "origin" "100 0 0"}"#,playsrc_entity::Limits::default()).unwrap();
        let runtime=Runtime::new(World::definitions(&graph).unwrap());
        let initial=runtime.committed.random.state();
        let (mut staged,mut candidate)=runtime.begin(1,0).unwrap();
        candidate.random.random_float(0.0,3.0);
        let advanced=candidate.random.state();
        staged.pending=Some((1,std::sync::Arc::new(candidate)));
        let (_,retry)=staged.begin(1,0).unwrap();assert_eq!(retry.random.state(),initial);
        let (accepted,next)=staged.begin(2,1).unwrap();assert_eq!(next.random.state(),advanced);
        assert_eq!(accepted.committed_frame,1);
        assert!(staged.begin(3,2).is_err());assert!(accepted.begin(1,0).is_err());
    }
    #[test]
    fn submitted_raster_results_drive_overlays_and_recreated_entities_drop_old_results() {
        let graph=playsrc_entity::parse(br#"{"classname" "worldspawn"}
          {"classname" "env_lightglow" "origin" "100 0 0" "HorizontalGlowSize" "8" "VerticalGlowSize" "4" "MaxDist" "100"}"#,playsrc_entity::Limits::default()).unwrap();
        let mut world=World::definitions(&graph).unwrap();
        let view=View::perspective([0.0;3],0.0,0.0,75.0,16.0/9.0,1.0,30000.0,720);
        let state=|generation|Some((playsrc_entity::EntityHandle{slot:1,generation},playsrc_entity::Transform{origin:[100.0,0.0,0.0],angles:[0.0;3]},
            playsrc_entity::EntityRenderState{brush_model:None,mode:0,color:[255;4],fx:0,effects:0},None));
        let mut client_frame=0;
        let mut draw=|now,feedback:&[Feedback],generation| {
            client_frame+=1;
            let mut bytes=Vec::new();world.frame(&view,0,now,client_frame,0.015,1.0,feedback,|_|state(generation),|_|None,|_|None,|_,_|(true,true),&mut bytes).unwrap();
            (u32::from_le_bytes(bytes[8..12].try_into().unwrap()),u32::from_le_bytes(bytes[12..16].try_into().unwrap()))
        };
        assert_eq!(draw(0.0,&[],1),(0,0));
        assert_eq!(draw(4.0,&[],1),(1,0));
        let result=Feedback{source:1,submission:1,visible:100,possible:100,clip_fraction:1.0};
        assert_eq!(draw(4.015,&[result],1),(1,1));
        assert_eq!(draw(4.03,&[Feedback{submission:2,visible:-1,possible:-1,..result}],1),(0,1));
        assert_eq!(draw(4.045,&[Feedback{submission:2,..result}],1),(1,1));
        assert_eq!(draw(5.0,&[Feedback{submission:2,..result}],2),(0,0));
        assert_eq!(draw(9.0,&[Feedback{submission:2,..result}],2),(1,0));
    }
}

#[derive(Clone)]
struct Glow {
    source: u32,
    definition: LightGlow,
    queries: [Query; 2],
    submissions: [u32; 2],
    discarded_submissions: [u32; 2],
    entity: Option<playsrc_entity::EntityHandle>,
    active: bool,
    next_think: f32,
    hdr_scale: f32,
}

#[derive(Clone,Default)]
struct SpriteClient {
    entity:Option<playsrc_entity::EntityHandle>,queries:[Query;5],submissions:[u32;5],discarded_submissions:[u32;5],fx:playsrc_map::render_fx::FxBlend,
}

#[derive(Clone)]
struct SunClient {entity:Option<playsrc_entity::EntityHandle>,queries:[[Query;2];2],cutoff:[[u32;2];2],sky_fraction:[f32;2]}
impl Default for SunClient {fn default()->Self{Self{entity:None,queries:[[Query::default();2];2],cutoff:[[0;2];2],sky_fraction:[1.0;2]}}}

struct DrawQuad {source:u32,material:u32,frame:u32,layer:u32,hdr:f32,origin:[f32;3],positions:[[f32;3];4],color:[[f32;4];4],uv:[[f32;2];4]}
struct DrawMesh {material:u32,sources:Vec<u32>,mesh:playsrc_beam::Mesh}
#[derive(Clone)]
struct RopeClient {definition:playsrc_entity::rope::Definition,physics:playsrc_map::legacy_rope::Rope,lighting:Arc<Vec<[f32;3]>>}
impl RopeClient{
    fn new(definition:playsrc_entity::rope::Definition,points:[Option<[f32;3]>;2],random:&mut playsrc_tf2::UniformRandomStream,lighting:&mut impl FnMut([f32;3])->Result<[f32;3],()>)->Result<Self,()>{
        if definition.flags&!(1|playsrc_entity::rope::SIMULATE|playsrc_entity::rope::NO_WIND|playsrc_entity::rope::INITIAL_HANG)!=0{eprintln!("Unsupported rope flags {} at {}",definition.flags,definition.source);return Err(());}
        let start=points[0].ok_or(())?;
        let mut physics=playsrc_map::legacy_rope::Rope::new([start,points[1].unwrap_or(start)],usize::from(definition.nodes),definition.length,definition.slack,definition.locked,definition.flags&playsrc_entity::rope::NO_WIND!=0,definition.flags&playsrc_entity::rope::INITIAL_HANG!=0,&mut |low,high|random.random_float(low,high));
        physics.set_endpoints(points,definition.locked);
        let lighting=Arc::new(physics.nodes.iter().map(|node|lighting(node.predicted)).collect::<Result<Vec<_>,_>>()?);
        Ok(Self{definition,physics,lighting})
    }
}

#[derive(Clone)]
pub struct World {
    glows: Vec<Glow>,
    layout:Arc<super::legacy_materials::Layout>,
    sprites:Vec<SpriteClient>,
    suns:Vec<SunClient>,
    spotlights:Vec<SpriteClient>,
    ropes:Vec<Option<RopeClient>>,
    screen_width:u32,samples:u32,
    random: playsrc_tf2::UniformRandomStream,
}

impl Default for World {
    fn default() -> Self { Self { glows:Vec::new(),layout:Arc::default(),sprites:Vec::new(),suns:Vec::new(),spotlights:Vec::new(),ropes:Vec::new(),screen_width:1280,samples:1,random:playsrc_tf2::UniformRandomStream::from_seed(0).expect("Source default random seed") } }
}

#[derive(Clone, Copy)]
pub struct Feedback {
    pub source: u32,
    pub submission: u32,
    pub visible: i32,
    pub possible: i32,
    pub clip_fraction: f32,
}

impl World {
    pub fn compile(graph:&playsrc_entity::Graph,bundle:&BTreeMap<String,&[u8]>,decoders:&super::TextureDecoders<'_>,profile:playsrc_map::LightingProfile)->Result<Self,()> {
        let mut world=Self::definitions(graph)?;
        world.layout=super::legacy_materials::compile(graph,bundle,decoders,profile)?;
        if !world.layout.ropes.is_empty()&&graph.entities.iter().any(|entity|entity.classname.as_deref().is_some_and(|class|class.eq_ignore_ascii_case(b"env_wind"))){eprintln!("Authored rope wind controller is unavailable");return Err(());}
        world.sprites=vec![SpriteClient::default();world.layout.sprites.len()];
        world.suns=vec![SunClient::default();world.layout.suns.len()];
        world.spotlights=vec![SpriteClient::default();world.layout.spotlights.len()];
        world.ropes=vec![None;world.layout.ropes.len()];
        Ok(world)
    }

    fn initialize_ropes(&mut self,mut state:impl FnMut(u32)->Option<(playsrc_entity::rope::Definition,[Option<[f32;3]>;2])>,mut lighting:impl FnMut([f32;3])->Result<[f32;3],()>)->Result<(),()>{
        for (layout,client) in self.layout.ropes.iter().zip(&mut self.ropes){
            let Some((definition,points))=state(layout.source).filter(|(definition,_)|definition.flags&playsrc_entity::rope::SIMULATE!=0)else{continue;};
            *client=Some(RopeClient::new(definition,points,&mut self.random,&mut lighting)?);
        }
        Ok(())
    }

    fn advance_ropes(&mut self,seconds:f32,view:[f32;3],mut state:impl FnMut(u32)->Option<(playsrc_entity::rope::Definition,[Option<[f32;3]>;2])>,mut lighting:impl FnMut([f32;3])->Result<[f32;3],()>)->Result<(),()>{
        for (layout,client) in self.layout.ropes.iter().zip(&mut self.ropes){
            let Some((definition,points))=state(layout.source).filter(|(definition,_)|definition.flags&playsrc_entity::rope::SIMULATE!=0)else{*client=None;continue;};
            if client.as_ref().is_none_or(|client|client.definition.entity!=definition.entity){
                *client=Some(RopeClient::new(definition.clone(),points,&mut self.random,&mut lighting)?);
            }
            let client=client.as_mut().ok_or(())?;
            if client.definition!=definition{client.physics.changed();}
            if client.definition.length!=definition.length||client.definition.slack!=definition.slack{client.physics.set_length(definition.length,definition.slack);}
            client.physics.set_endpoints(points,definition.locked);client.definition=definition;
            client.physics.advance(seconds,view,[0.0;3],&mut |low,high|self.random.random_float(low,high));
        }
        Ok(())
    }

    fn definitions(graph: &playsrc_entity::Graph) -> Result<Self, ()> {
        let mut world = Self::default();
        for entity in &graph.entities {
            if !entity.classname.as_deref().is_some_and(|class| class.eq_ignore_ascii_case(b"env_lightglow")) { continue; }
            let integer = |key| playsrc_entity::source_integer(super::entity_scalar(entity, key).unwrap_or_default());
            let radius = match super::entity_scalar(entity, b"GlowProxySize") {
                Some(value) => playsrc_entity::Variant::String(value.to_vec()).as_float().ok_or(())?, None => 2.0,
            };
            if !radius.is_finite() { return Err(()); }
            let hdr_scale=match super::entity_scalar(entity,b"HDRColorScale") {
                Some(value)=>playsrc_entity::Variant::String(value.to_vec()).as_float().ok_or(())?,None=>0.0,
            };
            world.glows.push(Glow { source: u32::try_from(entity.index).map_err(|_| ())?,
                definition: LightGlow { horizontal_size: integer(b"HorizontalGlowSize"), vertical_size: integer(b"VerticalGlowSize"),
                    minimum_distance: integer(b"MinDist"), maximum_distance: integer(b"MaxDist").min(65535),
                    outer_maximum_distance: integer(b"OuterMaxDist").min(65535), one_sided: integer(b"spawnflags") & 1 != 0,
                    proxy_radius: radius, distance_origin: super::entity_vector(entity, b"origin")? },
                queries: [Query::default(); 2], submissions: [0; 2], discarded_submissions:[0;2], entity:None, active:false, next_think:0.0, hdr_scale });
        }
        Ok(world)
    }

    pub fn frame(&mut self, view: &View, owner:u32, now: f32, client_frame:u32, client_frame_seconds:f32, fov_distance_adjust:f32,feedback: &[Feedback],
        mut state: impl FnMut(u32) -> Option<(playsrc_entity::EntityHandle, playsrc_entity::Transform, playsrc_entity::EntityRenderState,Option<playsrc_entity::sprite::Presentation>)>,
        mut sun_state:impl FnMut(u32)->Option<playsrc_entity::sun::Presentation>,
        mut spotlight_state:impl FnMut(u32)->Option<(playsrc_entity::spotlight::Beam,playsrc_entity::EntityRenderState)>,
        mut visibility: impl FnMut([f32; 3],Option<playsrc_visibility::Aabb>) -> (bool, bool), out: &mut Vec<u8>) -> Result<(), ()> {
        let camera=owner as usize;
        let frame=u64::from(client_frame);
        let mut proxies = Vec::new();
        let mut quads = Vec::new();
        let mut meshes:Vec<DrawMesh>=Vec::new();
        let feedback = feedback.iter().map(|value| (value.source,value)).collect::<BTreeMap<_,_>>();
        for glow in &mut self.glows {
            if owner>1 {continue;}
            let Some((entity, transform, render,_)) = state(glow.source) else { continue; };
            if glow.entity != Some(entity) {
                glow.entity=Some(entity); glow.definition.distance_origin=transform.origin;
                glow.queries=[Query::default();2]; glow.active=false;
                glow.discarded_submissions=glow.submissions;
                if let Some(value)=feedback.get(&glow.source) { glow.discarded_submissions[camera]=value.submission; }
                glow.next_think=now+self.random.random_float(0.0,3.0);
            }
            let (belongs,visible)=visibility(transform.origin,None);
            if !belongs { continue; }
            if now >= glow.next_think {
                glow.active=visible;
                glow.next_think=now+self.random.random_float(1.0,3.0);
            }
            if !glow.active { continue; }
            let query = &mut glow.queries[camera];
            if query.expired_before_frame(frame) {
                *query=Query::default();
                glow.discarded_submissions[camera]=feedback.get(&glow.source).map_or(glow.submissions[camera],|value|value.submission);
            }
            let counts = feedback.get(&glow.source).and_then(|value| {
                if value.submission <= glow.discarded_submissions[camera] { return None; }
                glow.submissions[camera] = value.submission;
                (value.visible >= 0 && value.possible >= 0).then_some((value.visible as u32,value.possible as u32))
            });
            let brightness = query.sample(frame, client_frame_seconds, DEFAULT_FADE_TIME, counts);
            let proxy=glow.definition.proxy(view,transform.origin);
            if query.issue(frame,proxy.as_ref()) && let Some(proxy)=proxy {proxies.push((glow.source,proxy));}
            let (sp,cp)=transform.angles[0].to_radians().sin_cos();
            let (sy,cy)=transform.angles[1].to_radians().sin_cos();
            if let Some(quad) = glow.definition.quad(view,transform.origin,[cp*cy,cp*sy,-sp],render.color[..3].try_into().unwrap(),brightness) {
                quads.push(DrawQuad{source:glow.source,material:0,frame:0,layer:2,hdr:glow.hdr_scale,origin:transform.origin,positions:quad.positions,color:[[quad.color[0],quad.color[1],quad.color[2],1.0];4],uv:[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]]});
            }
        }
        for (definition,client) in self.layout.suns.iter().zip(&mut self.suns){
            if owner>1{continue;}
            let Some(sun)=sun_state(definition.source).filter(|sun|sun.active) else {continue;};
            let Some((_,_,render,_))=state(definition.source) else{continue;};
            if client.entity!=Some(sun.entity){*client=SunClient::default();client.entity=Some(sun.entity);}
            let colors=playsrc_map::legacy_sun::colors(render.color[..3].try_into().unwrap(),sun.overlay_color[..3].try_into().unwrap());
            for layer in 0..2 {
                let identity=definition.source|if layer==0 {0x8000_0000}else{0x4000_0000};
                let query=&mut client.queries[layer][camera];
                if query.expired_before_frame(frame){*query=Query::default();client.cutoff[layer][camera]=feedback.get(&identity).map_or(0,|value|value.submission);}
                let counts=feedback.get(&identity).filter(|value|value.submission>client.cutoff[layer][camera]).and_then(|value|(value.visible>=0&&value.possible>=0).then_some((value.visible as u32,value.possible as u32)));
                let fraction=query.sample(frame,client_frame_seconds,DEFAULT_FADE_TIME,counts);
                let position=std::array::from_fn(|axis|view.origin[axis]+sun.direction[axis]*(view.far*0.999));
                let proxy=view.proxy(playsrc_map::pixel_visibility::Parameters{position,size:0.05,aspect:1.0,screen_space:true});
                if query.issue(frame,proxy.as_ref())&&let Some(proxy)=proxy{proxies.push((identity,proxy));}
                if owner==1 {client.sky_fraction[layer]=fraction;continue;}
                let size=if layer==0{sun.size}else{sun.overlay_size};
                if let Some(quad)=playsrc_map::legacy_sun::quad(view,sun.direction,size,colors[layer],fraction*client.sky_fraction[layer],layer==1){
                    quads.push(DrawQuad{source:definition.source,material:definition.materials[layer] as u32,frame:0,layer:2,hdr:sun.hdr_scale,origin:position,positions:quad.positions,color:[[quad.color[0],quad.color[1],quad.color[2],1.0];4],uv:[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]]});
                }
            }
        }
        for (definition,client) in self.layout.sprites.iter().zip(&mut self.sprites) {
            let Some((entity,transform,render,sprite))=state(definition.source) else {continue;};
            let sprite=sprite.ok_or(())?;if !sprite.active {continue;}
            let radius=definition.size[0].max(definition.size[1]) as f32*sprite.scale*0.5;
            if radius<0.0 {return Err(());}
            let bounds=playsrc_visibility::Aabb{minimum:transform.origin.map(|v|v-radius),maximum:transform.origin.map(|v|v+radius)};
            let (belongs,visible)=visibility(transform.origin,Some(bounds));if !belongs||!visible {continue;}
            if client.entity!=Some(entity) {
                *client=SpriteClient::default();client.entity=Some(entity);
                client.discarded_submissions[camera]=feedback.get(&definition.source).map_or(0,|value|value.submission);
            }
            let delta=std::array::from_fn::<_,3,_>(|axis|transform.origin[axis]-view.origin[axis]);
            let distance=delta[0]*view.forward[0]+delta[1]*view.forward[1]+delta[2]*view.forward[2];
            let blend=client.fx.sample(client_frame,u32::from(entity.slot),now,render.mode,render.fx,render.color,distance,255,|min,max|self.random.random_int(min,max).expect("Source FX range"));
            if blend==0 {continue;}
            let mut fraction=1.0;
            if render.mode==3||render.mode==9 {
                let query=&mut client.queries[camera];
                if query.expired_before_frame(frame) {*query=Query::default();client.discarded_submissions[camera]=feedback.get(&definition.source).map_or(client.submissions[camera],|value|value.submission);}
                let counts=feedback.get(&definition.source).and_then(|value|{
                    if value.submission<=client.discarded_submissions[camera] {return None;}
                    client.submissions[camera]=value.submission;
                    (value.visible>=0&&value.possible>=0).then_some((value.visible as u32,value.possible as u32))
                });
                fraction=query.sample(frame,client_frame_seconds,DEFAULT_FADE_TIME,counts);
                let proxy=view.proxy(playsrc_map::pixel_visibility::Parameters{position:transform.origin,size:definition.proxy_radius,aspect:definition.size[0] as f32/definition.size[1] as f32,screen_space:false});
                if query.issue(frame,proxy.as_ref())&&let Some(proxy)=proxy {proxies.push((definition.source,proxy));}
            }
            let color=[render.color[0],render.color[1],render.color[2],sprite.brightness];
            let Some(quad)=playsrc_map::legacy_sprite::quad(view,transform.origin,transform.angles,definition.orientation,definition.extents,definition.size,sprite.scale,render.mode,render.fx,color,blend,fraction,fov_distance_adjust) else {continue;};
            for (pass,&material) in definition.variants.get(usize::from(render.mode)).ok_or(())?.iter().enumerate() {
                if self.layout.materials[material].state.no_draw {continue;}
                let requested=sprite.frame as i32;
                let requested=if render.mode==8&&pass==1 {(requested+1)%(definition.frames as i32)} else {requested};
                let selected=if requested<0||requested as u32>=definition.frames {0}else{requested as u32};
                let program=&self.layout.materials[material].program;
                quads.push(DrawQuad{source:definition.source,material:material as u32,frame:selected,layer:if render.mode==3||render.mode==9 {1}else{0},
                    hdr:if program.hdr_gamma {playsrc_material::legacy_sprite::gamma_constant(definition.hdr_scale)}else{definition.hdr_scale},
                    origin:transform.origin,positions:quad.positions,color:[quad.color.map(|value|f32::from(value)/255.0);4],uv:quad.uv});
            }
        }
        for (definition,client) in self.layout.spotlights.iter().zip(&mut self.spotlights){
            let Some((beam,render))=spotlight_state(definition.source) else{continue;};
            if render.effects&0x20!=0||beam.minimum_dx_level>95{continue;}
            let radius=0.5*beam.width.max(beam.end_width);
            let bounds=playsrc_visibility::Aabb{minimum:std::array::from_fn(|i|beam.start[i].min(beam.end[i])-radius),maximum:std::array::from_fn(|i|beam.start[i].max(beam.end[i])+radius)};
            let (belongs,visible)=visibility(beam.start,Some(bounds));if !belongs||!visible{continue;}
            if client.entity!=Some(beam.entity){*client=SpriteClient::default();client.entity=Some(beam.entity);client.discarded_submissions[camera]=feedback.get(&definition.source).map_or(0,|value|value.submission);}
            let blend=client.fx.sample(client_frame,u32::from(beam.entity.slot),now,render.mode,render.fx,render.color,0.0,255,|min,max|self.random.random_int(min,max).expect("Source FX range"));
            if blend==0{continue;}
            let query=&mut client.queries[camera];
            if query.expired_before_frame(frame){*query=Query::default();client.discarded_submissions[camera]=feedback.get(&definition.source).map_or(client.submissions[camera],|value|value.submission);}
            let counts=feedback.get(&definition.source).filter(|value|value.submission>client.discarded_submissions[camera]).and_then(|value|(value.visible>=0&&value.possible>=0).then_some((value.visible as u32,value.possible as u32)));
            let fraction=query.sample(frame,client_frame_seconds,DEFAULT_FADE_TIME,counts);
            let color=[render.color[0],render.color[1],render.color[2],blend];
            let Some(geometry)=playsrc_map::legacy_spotlight::geometry(view,beam.start,beam.end,beam.width,beam.end_width,color,fraction)else{continue;};
            let proxy=view.proxy(playsrc_map::pixel_visibility::Parameters{position:beam.start,size:geometry.halo_proxy_size,aspect:1.0,screen_space:false});
            if query.issue(frame,proxy.as_ref())&&let Some(proxy)=proxy{proxies.push((definition.source,proxy));}
            for (index,quad) in std::iter::once(geometry.beam).chain(geometry.halo).enumerate(){
                let asset=definition.materials[index];let hdr=if self.layout.materials[asset].program.hdr_gamma{playsrc_material::legacy_sprite::gamma_constant(beam.hdr_scale)}else{beam.hdr_scale};
                quads.push(DrawQuad{source:definition.source,material:asset as u32,frame:0,layer:0,hdr,origin:std::array::from_fn(|i|(beam.start[i]+beam.end[i])*0.5),positions:quad.positions,color:quad.colors,uv:quad.uv});
            }
        }
        for (layout,client) in self.layout.ropes.iter().zip(&self.ropes){
            let Some(client)=client else{continue;};let Some((_,transform,render,_))=state(layout.source)else{continue;};
            if render.effects&0x20!=0{continue;}
            let mut minimum=client.physics.nodes[0].position;let mut maximum=minimum;
            for node in &client.physics.nodes{for axis in 0..3{minimum[axis]=minimum[axis].min(node.position[axis]);maximum[axis]=maximum[axis].max(node.position[axis]);}}
            let (belongs,visible)=visibility(transform.origin,Some(playsrc_visibility::Aabb{minimum,maximum}));if !belongs||!visible{continue;}
            let definition=&client.definition;
            let geometry=playsrc_map::legacy_rope::geometry(&client.physics,playsrc_map::legacy_rope::Draw{lighting:&client.lighting,color_modulation:[1.0;3],width:definition.width,
                subdivisions:usize::from(if definition.subdivisions==255{2}else{definition.subdivisions.min(7)}),length:definition.length,slack:definition.slack,texture_scale:definition.texture_scale,mapping_height:layout.mapping_height,
                view,screen_width:self.screen_width,samples:self.samples,has_back:layout.back.is_some()});
            for (material,mesh) in [(layout.back,geometry.back),(Some(layout.material),geometry.solid)]{
                let (Some(material),Some(mesh))=(material,mesh)else{continue;};
                if let Some(batch)=meshes.iter_mut().find(|batch|batch.material==material as u32){
                    let base=batch.mesh.vertices.len() as u32;batch.mesh.vertices.extend(mesh.vertices);batch.mesh.indices.extend(mesh.indices.into_iter().map(|index|index+base));batch.sources.push(layout.source);
                }else{meshes.push(DrawMesh{material:material as u32,sources:vec![layout.source],mesh});}
            }
        }
        out.extend_from_slice(b"PLVF"); out.extend_from_slice(&8_u32.to_le_bytes());
        out.extend_from_slice(&(proxies.len() as u32).to_le_bytes()); out.extend_from_slice(&(quads.len() as u32).to_le_bytes());
        out.extend_from_slice(&(meshes.len() as u32).to_le_bytes());
        for (source,proxy) in proxies {
            out.extend_from_slice(&source.to_le_bytes()); out.extend_from_slice(&proxy.clip_fraction.to_le_bytes());
            for vertex in proxy.vertices {for value in vertex.into_iter().chain([1.0]) {out.extend_from_slice(&value.to_le_bytes());}}
        }
        for quad in quads {
            for value in [quad.source,quad.material,quad.frame,quad.layer] {out.extend_from_slice(&value.to_le_bytes());}
            out.extend_from_slice(&quad.hdr.to_le_bytes());
            for value in quad.origin.into_iter().chain(quad.positions.into_iter().flatten()).chain(quad.color.into_iter().flatten()).chain(quad.uv.into_iter().flatten()) {out.extend_from_slice(&value.to_le_bytes());}
        }
        for batch in meshes{
            for value in [batch.material,batch.sources.len() as u32,batch.mesh.vertices.len() as u32,batch.mesh.indices.len() as u32]{out.extend_from_slice(&value.to_le_bytes());}
            for source in batch.sources{out.extend_from_slice(&source.to_le_bytes());}
            for vertex in batch.mesh.vertices{for value in vertex.position.into_iter().chain(vertex.uv){out.extend_from_slice(&value.to_le_bytes());}out.extend_from_slice(&vertex.color);}
            for index in batch.mesh.indices{out.extend_from_slice(&index.to_le_bytes());}
        }
        Ok(())
    }
}

#[derive(Clone,Default)]
pub struct Runtime {
    committed_frame:u32,
    committed:std::sync::Arc<World>,
    pending:Option<(u32,std::sync::Arc<World>)>,
}

#[cfg(not(target_arch="wasm32"))]
pub struct RopeFacts {pub source:u32,pub nodes:Vec<[f32;3]>,pub no_wind:bool,pub material:String,pub cameras:Vec<[f32;5]>}

impl Runtime {
    pub fn new(world:World)->Self {Self{committed:std::sync::Arc::new(world),..Default::default()}}
    pub fn required(&self)->bool {!self.committed.glows.is_empty()||!self.committed.layout.sprites.is_empty()||!self.committed.layout.suns.is_empty()||!self.committed.layout.spotlights.is_empty()||!self.committed.layout.ropes.is_empty()}
    pub fn initialize_ropes(&mut self,map:&playsrc_tf2::MapRuntime,lighting:&mut playsrc_map::ModelLightingWorld<'_>,visibility:&playsrc_visibility::World,collision:&playsrc_collision::World,snapshot:&playsrc_collision::Snapshot)->Result<(),()>{
        Arc::make_mut(&mut self.committed).initialize_ropes(|source|map.rope_state(source),|position|lighting.point_lighting(position,visibility,collision,snapshot).inspect_err(|_|eprintln!("Rope lighting at {position:?}")))
    }
    #[cfg(not(target_arch="wasm32"))]
    pub fn rope_facts(&self)->Vec<RopeFacts>{self.committed.ropes.iter().flatten().map(|rope|RopeFacts{source:rope.definition.source,nodes:rope.physics.nodes.iter().map(|node|node.predicted).collect(),no_wind:rope.definition.flags&playsrc_entity::rope::NO_WIND!=0,material:rope.definition.material.to_string(),cameras:Vec::new()}).collect()}
    fn begin(&self,client_frame:u32,accepted_client_frame:u32)->Result<(Self,World),()> {
        if client_frame!=accepted_client_frame.checked_add(1).ok_or(())? {return Err(());}
        let mut runtime=self.clone();
        if accepted_client_frame!=runtime.committed_frame {
            let (frame,world)=runtime.pending.take().filter(|(frame,_)|*frame==accepted_client_frame).ok_or(())?;
            runtime.committed_frame=frame;runtime.committed=world;
        }
        runtime.pending=None;
        let candidate=(*runtime.committed).clone();Ok((runtime,candidate))
    }
}

/// Produce one speculative client-frame result. The caller publishes this state
/// and all visual/particle output ranges atomically only after both producers
/// succeed. Only a later accepted-frame acknowledgement commits the candidate.
pub(super) fn prepare(slot:&super::Slot,payload:&[u8],client_frame:u32,accepted_client_frame:u32,client_frame_seconds:f32)->Result<(Runtime,Vec<u8>),()> {
    if !client_frame_seconds.is_finite()||client_frame_seconds<0.0 {return Err(());}
    let (mut runtime,mut candidate)=slot.legacy_visuals.begin(client_frame,accepted_client_frame)?;
    let (Some(visibility),Some(area),Some(candidates),Some(environment),Some(session))=(slot.visibility.as_ref(),slot.area_state.as_ref(),slot.visibility_candidates.as_ref(),slot.environment.as_ref(),slot.session.as_ref()) else {return Err(());};
    let mut reader=Reader{bytes:payload,at:0};
    if reader.take(4)?!=b"PLVQ"||reader.u32()?!=3 {return Err(());}
    let count=reader.u32()?;
    candidate.screen_width=reader.u32()?;candidate.samples=reader.u32()?;
    if !(1..=32768).contains(&candidate.screen_width)||![1,4].contains(&candidate.samples){return Err(());}
    if !(1..=5).contains(&count) {return Err(());}
    let mut scan=Reader{bytes:payload,at:reader.at};let mut main_origin=None;
    for _ in 0..count{
        let owner=scan.u32()?;scan.take(8)?;let feedback=scan.u32()?;let position=[scan.f32()?,scan.f32()?,scan.f32()?];scan.take(36)?;
        scan.take((feedback as usize).checked_mul(20).ok_or(())?)?;if owner==0{main_origin=Some(position);}
    }
    let mut lighting=slot.model_lighting_world.as_ref().ok_or(())?.borrowed_view();
    let world=slot.gameplay_world.as_ref().ok_or(())?;let snapshot=world.snapshot();
    candidate.advance_ropes(client_frame_seconds,main_origin.ok_or(())?,|source|session.map_rope_state(source),|position|lighting.point_lighting(position,visibility,&world.world,&snapshot))?;
    let mut owners=std::collections::BTreeSet::new();
    let mut output=b"PLVF".to_vec();output.extend_from_slice(&5_u32.to_le_bytes());output.extend_from_slice(&count.to_le_bytes());
    let sky_area=environment.world.controllers.iter().find_map(|controller|match controller.state {playsrc_map::ControllerState::SkyCamera{area,..}=>Some(area),_=>None});
    for _ in 0..count {
        let owner=reader.u32()?;if owner>4||!owners.insert(owner) {return Err(());}
        let now=reader.f32()?;let height=reader.u32()?;let feedback_count=reader.u32()?;
        if now<0.0||!(1..=32768).contains(&height)||feedback_count>65536 {return Err(());}
        let position=[reader.f32()?,reader.f32()?,reader.f32()?];
        let pvs_origin=[reader.f32()?,reader.f32()?,reader.f32()?];
        let yaw=reader.f32()?;let pitch=reader.f32()?;let fov=reader.f32()?;let aspect=reader.f32()?;let near=reader.f32()?;let far=reader.f32()?;
        if fov<=0.0||fov>=180.0||aspect<=0.0||near<=0.0||far<=near {return Err(());}
        let mut feedback=Vec::with_capacity(feedback_count as usize);
        let mut sources=std::collections::BTreeSet::new();
        for _ in 0..feedback_count {
            let value=Feedback{source:reader.u32()?,submission:reader.u32()?,visible:reader.u32()? as i32,possible:reader.u32()? as i32,clip_fraction:reader.f32()?};
            if value.submission==0||value.visible < -1||value.possible < -1||!(0.0..=1.0).contains(&value.clip_fraction)||!sources.insert(value.source) {return Err(());}
            feedback.push(value);
        }
        let pvs=visibility.view(area,candidates,&playsrc_visibility::ViewQuery{origins:vec![pvs_origin],bypass_pvs:false}).map_err(|_|())?;
        let view=View::perspective(position,yaw,pitch,fov,aspect,near,far,height);
        let mut bytes=Vec::new();
        let fov_distance_adjust=if slot.latest_game_snapshot.as_ref().is_some_and(|snapshot|snapshot.class==playsrc_tf2::PlayerClass::Sniper&&snapshot.weapon==Some(playsrc_tf2::Weapon::SniperRifle)&&snapshot.conditions&2!=0) {20.0/75.0} else {1.0};
        candidate.frame(&view,owner,now,client_frame,client_frame_seconds,fov_distance_adjust,&feedback,|source|{
            let (entity,transform,render)=session.map_visual_entity(source)?;Some((entity,transform,render,session.map_sprite_state(source)))
        },|source|session.map_sun_state(source),|source|session.map_spotlight_state(source),|position,bounds|{
            visibility.locate_leaf(position).ok().map_or((false,false),|leaf| {
                let sky=Some(usize::from(visibility.leaves[leaf].area_and_flags&0x1ff))==sky_area;
                let visible=bounds.map_or_else(||pvs.leaves.contains(&leaf),|bounds|visibility.leaves_in_box(bounds).is_ok_and(|leaves|leaves.iter().any(|leaf|pvs.leaves.contains(leaf))));
                (sky==(owner==1),visible)
            })
        },&mut bytes)?;
        output.extend_from_slice(&owner.to_le_bytes());output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());output.extend_from_slice(&bytes);
    }
    if reader.at!=payload.len()||!owners.contains(&0) {return Err(());}
    if output.len()>4*1024*1024 {return Err(());}
    runtime.pending=Some((client_frame,std::sync::Arc::new(candidate)));
    Ok((runtime,output))
}

struct Reader<'a>{bytes:&'a[u8],at:usize}
impl<'a> Reader<'a>{
    fn take(&mut self,count:usize)->Result<&'a[u8],()>{let end=self.at.checked_add(count).ok_or(())?;let bytes=self.bytes.get(self.at..end).ok_or(())?;self.at=end;Ok(bytes)}
    fn u32(&mut self)->Result<u32,()>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
    fn f32(&mut self)->Result<f32,()>{let value=f32::from_bits(self.u32()?);if value.is_finite(){Ok(value)}else{Err(())}}
}
