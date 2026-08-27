use std::collections::BTreeMap;
use playsrc_map::{legacy_glow::LightGlow, pixel_visibility::{Query, View, DEFAULT_FADE_TIME}};

pub const GLOW_MATERIAL: &str = "materials/sprites/light_glow02_add_noz.vmt";

pub fn encode_materials(out: &mut Vec<u8>, graph: &playsrc_entity::Graph, bundle: &BTreeMap<String, &[u8]>,
    decoders: &super::TextureDecoders<'_>, hashes: &BTreeMap<String,[u8;32]>, profile: playsrc_map::LightingProfile) -> Result<(), ()> {
    let world = World::compile(graph)?;
    let materials = world.materials().collect::<Vec<_>>();
    out.extend_from_slice(b"PLVM"); out.extend_from_slice(&1_u32.to_le_bytes()); out.extend_from_slice(&(materials.len() as u32).to_le_bytes());
    for identity in materials {
        let material = super::resolve_material_semantics(identity, bundle, super::material_environment(profile,false))?;
        let (selected,_,_) = super::selected_texture(&material,decoders)?;
        let path=selected.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
        let texture=super::model_authored_texture(&path,decoders,hashes,true)?;
        super::pbytes(out,identity.as_bytes())?;
        super::encode_model_authored_texture(out,&path,&texture)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandoned_render_candidates_do_not_advance_the_source_random_stream() {
        let graph=playsrc_entity::parse(br#"{"classname" "env_lightglow" "origin" "100 0 0"}"#,playsrc_entity::Limits::default()).unwrap();
        let runtime=Runtime::new(World::compile(&graph).unwrap());
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
        let mut world=World::compile(&graph).unwrap();
        let view=View::perspective([0.0;3],0.0,0.0,75.0,16.0/9.0,1.0,30000.0,720);
        let state=|generation|Some((playsrc_entity::EntityHandle{slot:1,generation},playsrc_entity::Transform{origin:[100.0,0.0,0.0],angles:[0.0;3]},
            playsrc_entity::EntityRenderState{brush_model:None,mode:0,color:[255;4],fx:0,effects:0}));
        let mut client_frame=0;
        let mut draw=|now,feedback:&[Feedback],generation| {
            client_frame+=1;
            let mut bytes=Vec::new();world.frame(&view,false,now,client_frame,0.015,feedback,|_|state(generation),|_|(true,true),&mut bytes).unwrap();
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

#[derive(Clone)]
pub struct World {
    glows: Vec<Glow>,
    random: playsrc_tf2::UniformRandomStream,
}

impl Default for World {
    fn default() -> Self { Self { glows:Vec::new(),random:playsrc_tf2::UniformRandomStream::from_seed(0).expect("Source default random seed") } }
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
    pub fn compile(graph: &playsrc_entity::Graph) -> Result<Self, ()> {
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

    pub fn materials(&self) -> impl Iterator<Item=&'static str> { (!self.glows.is_empty()).then_some(GLOW_MATERIAL).into_iter() }

    pub fn frame(&mut self, view: &View, sky: bool, now: f32, client_frame:u32, client_frame_seconds:f32, feedback: &[Feedback],
        mut state: impl FnMut(u32) -> Option<(playsrc_entity::EntityHandle, playsrc_entity::Transform, playsrc_entity::EntityRenderState)>,
        mut visibility: impl FnMut([f32; 3]) -> (bool, bool), out: &mut Vec<u8>) -> Result<(), ()> {
        let camera = usize::from(sky);
        let frame=u64::from(client_frame);
        let mut proxies = Vec::new();
        let mut quads = Vec::new();
        let feedback = feedback.iter().map(|value| (value.source,value)).collect::<BTreeMap<_,_>>();
        for glow in &mut self.glows {
            let Some((entity, transform, render)) = state(glow.source) else { continue; };
            if glow.entity != Some(entity) {
                glow.entity=Some(entity); glow.definition.distance_origin=transform.origin;
                glow.queries=[Query::default();2]; glow.active=false;
                glow.discarded_submissions=glow.submissions;
                if let Some(value)=feedback.get(&glow.source) { glow.discarded_submissions[camera]=value.submission; }
                glow.next_think=now+self.random.random_float(0.0,3.0);
            }
            let (belongs,visible)=visibility(transform.origin);
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
                quads.push((glow.source,glow.hdr_scale,quad));
            }
        }
        out.extend_from_slice(b"PLVF"); out.extend_from_slice(&2_u32.to_le_bytes());
        out.extend_from_slice(&(proxies.len() as u32).to_le_bytes()); out.extend_from_slice(&(quads.len() as u32).to_le_bytes());
        for (source,proxy) in proxies {
            out.extend_from_slice(&source.to_le_bytes()); out.extend_from_slice(&proxy.clip_fraction.to_le_bytes());
            for value in proxy.clip_vertices.into_iter().flatten() { out.extend_from_slice(&value.to_le_bytes()); }
        }
        for (source,hdr_scale,quad) in quads {
            out.extend_from_slice(&source.to_le_bytes()); out.extend_from_slice(&0_u32.to_le_bytes());
            out.extend_from_slice(&0_u32.to_le_bytes());out.extend_from_slice(&hdr_scale.to_le_bytes());
            for value in quad.positions.into_iter().flatten().chain(quad.color).chain([1.0]) { out.extend_from_slice(&value.to_le_bytes()); }
            for value in [0.0_f32,1.0,1.0,1.0,1.0,0.0,0.0,0.0] {out.extend_from_slice(&value.to_le_bytes());}
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

impl Runtime {
    pub fn new(world:World)->Self {Self{committed:std::sync::Arc::new(world),..Default::default()}}
    pub fn required(&self)->bool {!self.committed.glows.is_empty()}
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
    if reader.take(4)?!=b"PLVQ"||reader.u32()?!=1 {return Err(());}
    let count=reader.u32()?;
    if !(1..=2).contains(&count) {return Err(());}
    let mut output=b"PLVF".to_vec();output.extend_from_slice(&3_u32.to_le_bytes());output.extend_from_slice(&count.to_le_bytes());
    let sky_area=environment.world.controllers.iter().find_map(|controller|match controller.state {playsrc_map::ControllerState::SkyCamera{area,..}=>Some(area),_=>None});
    for index in 0..count {
        let owner=reader.u32()?;if owner!=index {return Err(());}
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
        candidate.frame(&view,owner==1,now,client_frame,client_frame_seconds,&feedback,|source|session.map_visual_entity(source),|position|{
            visibility.locate_leaf(position).ok().map_or((false,false),|leaf| {
                let sky=Some(usize::from(visibility.leaves[leaf].area_and_flags&0x1ff))==sky_area;
                (sky==(owner==1),pvs.leaves.contains(&leaf))
            })
        },&mut bytes)?;
        output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());output.extend_from_slice(&bytes);
    }
    if reader.at!=payload.len() {return Err(());}
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
