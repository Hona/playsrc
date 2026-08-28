use std::{collections::BTreeMap,sync::Arc};
use playsrc_material::{legacy_sprite::{Orientation,SpriteMaterial,gamma_constant},Material,Shader,StaticState,TextureAlphaFacts,TextureColorRead};

#[derive(Clone)]
pub struct Program {pub srgb:bool,pub vertex_rgb:bool,pub vertex_alpha:bool,pub vertex_gamma:bool,pub gamma_exposure:bool,pub hdr_gamma:bool,pub world_renderable:bool,pub modulation:[f32;4],pub cable:bool}

pub struct Asset {pub identity:String,pub source:String,pub texture:String,pub state:StaticState,pub program:Program,pub normal:Option<String>}

pub struct SpriteDefinition {
    pub source:u32,pub size:[u32;2],pub frames:u32,pub orientation:Orientation,pub extents:[f32;4],pub proxy_radius:f32,pub hdr_scale:f32,
    pub variants:[Vec<usize>;11],
}
pub struct SunDefinition {pub source:u32,pub materials:[usize;2]}
pub struct SpotlightDefinition {pub source:u32,pub materials:[usize;2]}
pub struct RopeDefinition {pub source:u32,pub material:usize,pub back:Option<usize>,pub mapping_height:u32}

#[derive(Default)]
pub struct Layout {pub materials:Vec<Asset>,pub sprites:Vec<SpriteDefinition>,pub suns:Vec<SunDefinition>,pub spotlights:Vec<SpotlightDefinition>,pub ropes:Vec<RopeDefinition>}

fn resolved(path:&str,bundle:&BTreeMap<String,&[u8]>,decoders:&super::TextureDecoders<'_>,profile:playsrc_map::LightingProfile,entity_binding:bool)->Result<(Material,String,StaticState,[u32;2],u32),()> {
    let material=super::resolve_material_semantics(path,bundle,super::material_environment(profile,false))?;
    // Beam/overlay Draw_SetSpriteTexture binds without a renderable. The lamp
    // proxy is entity-only and performs no operation on that binding path.
    if material.proxies.iter().any(|proxy|entity_binding||!proxy.name.eq_ignore_ascii_case(b"lampbeam")) {eprintln!("Legacy visual material {path} requires proxy execution");return Err(());}
    let (selected,_,metadata)=super::selected_texture(&material,decoders)?;
    let texture=selected.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
    let state=playsrc_material::static_state(&material,TextureAlphaFacts{base:metadata.alpha_flags.one_bit||metadata.alpha_flags.eight_bit}).map_err(|_|())?;
    Ok((material,texture,state,[metadata.width,metadata.height],u32::from(metadata.frame_count)))
}

fn unlit_program(material:&Material)->Result<Program,()> {
    if material.shader!=Shader::UnlitGeneric||material.detail.is_some()||material.environment_map.is_some() {eprintln!("Unsupported legacy unlit inputs: {:?}",material.shader_token);return Err(());}
    let srgb=match material.textures.iter().find(|texture|texture.role==playsrc_material::TextureRole::Base).ok_or(())?.color_read {
        TextureColorRead::Srgb=>true,TextureColorRead::Linear=>false,TextureColorRead::FormatDependent=>{eprintln!("Legacy unlit texture needs format-dependent color selection");return Err(())},
    };
    Ok(Program{srgb,vertex_rgb:material.features.vertex_color,vertex_alpha:material.features.vertex_alpha,vertex_gamma:false,gamma_exposure:false,hdr_gamma:false,world_renderable:false,cable:false,
        modulation:playsrc_material::legacy_sprite::unlit_sprite_modulation(material).map_err(|_|())?})
}

fn render_program(material:&Material,sprite:Option<&SpriteMaterial>,mut state:StaticState,mode:u8,pass_index:usize)->Result<(StaticState,Program),()> {
    let mut program=if let Some(sprite)=sprite {
        let pass=*sprite.passes(mode).map_err(|_|())?.get(pass_index).ok_or(())?;
        state.lighting=playsrc_material::LightingModel::Unlit;state.blend=pass.blend;state.depth_test=pass.depth_test;state.depth_write=pass.depth_write;state.cull=pass.cull;state.fog=pass.fog;
        state.alpha_test=false;state.fragment_discard=playsrc_material::FragmentDiscardRequirement::None;state.alpha_modulation=1.0;
        state.alpha_ownership.opacity=true;state.vertex_color=pass.vertex_color;state.vertex_alpha=pass.vertex_color;state.translucent_queue=pass.blend.enabled;
        let mut modulation=[1.0;4];
        if pass.constant_color {
            modulation=if mode==8 {let weight=if pass_index==0 {sprite.alpha}else{0.0};[weight,weight,weight,1.0]}else{[sprite.color[0],sprite.color[1],sprite.color[2],sprite.alpha]};
            if sprite.srgb {for color in &mut modulation[..3] {*color=gamma_constant(*color);}}
        }
        Program{srgb:sprite.srgb,vertex_rgb:pass.vertex_color,vertex_alpha:pass.vertex_color,vertex_gamma:sprite.srgb,gamma_exposure:!sprite.srgb,hdr_gamma:sprite.srgb,world_renderable:true,modulation,cable:false}
    }else{unlit_program(material)?};
    program.world_renderable=true;Ok((state,program))
}

pub fn compile(graph:&playsrc_entity::Graph,bundle:&BTreeMap<String,&[u8]>,decoders:&super::TextureDecoders<'_>,profile:playsrc_map::LightingProfile)->Result<Arc<Layout>,()> {
    let mut layout=Layout::default();let mut variants=BTreeMap::new();
    if graph.entities.iter().any(|entity|entity.classname.as_deref().is_some_and(|class|class.eq_ignore_ascii_case(b"env_lightglow"))) {
        let source=super::legacy_visuals::GLOW_MATERIAL;
        let (material,texture,state,_,_)=resolved(source,bundle,decoders,profile,false)?;
        layout.materials.push(Asset{identity:source.into(),source:source.into(),texture,state,program:unlit_program(&material)?,normal:None});
    }
    for entity in &graph.entities {
        if entity.classname.as_deref().is_some_and(playsrc_entity::rope::is_rope){
            let source=playsrc_entity::rope::material(entity).map_err(|_|())?;
            let mut add=|source:&str|->Result<(usize,u32),()>{
                if let Some(index)=layout.materials.iter().position(|asset|asset.identity==format!("{source}#rope")){
                    let (_,_,_,size,_)=resolved(source,bundle,decoders,profile,false)?;return Ok((index,size[1]));
                }
                let (material,texture,mut state,size,_)=resolved(source,bundle,decoders,profile,false).inspect_err(|_|eprintln!("Rope material resolution failed: {source}"))?;
                if material.shader!=Shader::Cable{eprintln!("Rope shader is not Cable: {source} {:?}",material.shader_token);return Err(());}
                let normal=material.textures.iter().find(|texture|texture.role==playsrc_material::TextureRole::Bump).and_then(|texture|texture.logical_path.clone()).ok_or(())?;
                state.vertex_color=true;state.vertex_alpha=true;
                let index=layout.materials.len();
                layout.materials.push(Asset{identity:format!("{source}#rope"),source:source.into(),texture,state,normal:Some(normal),
                    program:Program{srgb:true,vertex_rgb:true,vertex_alpha:true,vertex_gamma:false,gamma_exposure:false,hdr_gamma:false,world_renderable:true,modulation:[1.0;4],cable:true}});
                Ok((index,size[1]))
            };
            let (material,mapping_height)=add(&source)?;
            let back_source=format!("{}_back.vmt",source.strip_suffix(".vmt").ok_or(())?);
            let back=if bundle.contains_key(&back_source){Some(add(&back_source)?.0)}else{None};
            layout.ropes.push(RopeDefinition{source:entity.index as u32,material,back,mapping_height});
        }
        if entity.classname.as_deref().is_some_and(|class|class.eq_ignore_ascii_case(b"point_spotlight")){
            let mut materials=[0;2];
            for (index,(source,mode)) in [("materials/sprites/glow_test02.vmt",5),("materials/sprites/light_glow03.vmt",3)].into_iter().enumerate(){
                let identity=format!("{source}#spotlight={mode}");
                materials[index]=if let Some(index)=layout.materials.iter().position(|asset|asset.identity==identity){index}else{
                    let (material,texture,state,_,_)=resolved(source,bundle,decoders,profile,false)?;
                    let sprite=if material.shader==Shader::Sprite {Some(SpriteMaterial::compile(&material).map_err(|_|())?)}else{None};
                    let (state,program)=render_program(&material,sprite.as_ref(),state,mode,0)?;let index=layout.materials.len();
                    layout.materials.push(Asset{identity,source:source.into(),texture,state,program,normal:None});index
                };
            }
            layout.spotlights.push(SpotlightDefinition{source:entity.index as u32,materials});
        }
        if entity.classname.as_deref().is_some_and(|class|class.eq_ignore_ascii_case(b"env_sun")){
            let sources=playsrc_entity::visual_resources::sun_materials(entity).map_err(|_|())?;
            let mut materials=[0;2];
            for (index,source) in sources.into_iter().enumerate(){
                materials[index]=if let Some(index)=layout.materials.iter().position(|asset|asset.identity==source){index}else{
                    let (material,texture,state,_,_)=resolved(&source,bundle,decoders,profile,false)?;let index=layout.materials.len();
                    layout.materials.push(Asset{identity:source.clone(),source,texture,state,program:unlit_program(&material)?,normal:None});index
                };
            }
            layout.suns.push(SunDefinition{source:entity.index as u32,materials});
        }
        if !entity.classname.as_deref().is_some_and(playsrc_entity::sprite::is_sprite) {continue;}
        let model=std::str::from_utf8(super::entity_scalar(entity,b"model").ok_or(())?).map_err(|_|())?;
        let source=playsrc_entity::visual_resources::sprite_material(model).ok_or(())?;
        let (material,texture,state,size,frames)=resolved(&source,bundle,decoders,profile,true).inspect_err(|_|eprintln!("Legacy sprite resource resolution: {source}"))?;
        let sprite=if material.shader==Shader::Sprite {Some(SpriteMaterial::compile(&material).map_err(|error|{eprintln!("Legacy sprite material {source}: {error}");})?)} else {None};
        let orientation=sprite.as_ref().map_or(Orientation::ParallelUpright,|sprite|sprite.orientation);
        let extents=sprite.as_ref().map_or([-0.5*size[0] as f32,0.5*size[0] as f32,0.5*size[1] as f32,-0.5*size[1] as f32],|sprite|sprite.extents(size[0] as f32,size[1] as f32));
        let mut modes:[Vec<usize>;11]=std::array::from_fn(|_|Vec::new());
        for mode in 0..11_u8 {
            if mode==6||mode==10 {continue;}
            let passes=sprite.as_ref().map(|sprite|sprite.passes(mode)).transpose().map_err(|_|())?;
            for pass_index in 0..passes.as_ref().map_or(1,Vec::len) {
                let key=(source.clone(),if sprite.is_some(){mode}else{255},pass_index);
                let index=if let Some(index)=variants.get(&key) {*index} else {
                    let (state,program)=render_program(&material,sprite.as_ref(),state,mode,pass_index)?;
                    let index=layout.materials.len();
                    layout.materials.push(Asset{identity:format!("{source}#sprite={mode}:{pass_index}"),source:source.clone(),texture:texture.clone(),state,program,normal:None});variants.insert(key,index);index
                };
                modes[usize::from(mode)].push(index);
            }
        }
        let number=|key:&[u8],default|super::entity_scalar(entity,key).map(|value|playsrc_entity::Variant::String(value.to_vec()).as_float().ok_or(())).unwrap_or(Ok(default));
        layout.sprites.push(SpriteDefinition{source:entity.index as u32,size,frames,orientation,extents,proxy_radius:number(b"GlowProxySize",2.0)?,hdr_scale:number(b"HDRColorScale",1.0)?,variants:modes});
    }
    Ok(Arc::new(layout))
}
