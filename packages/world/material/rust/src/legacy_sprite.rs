//! Sprite_DX9 material initialization and authored render-mode passes.
use crate::{BlendFactor, BlendState, CullState, Error, ErrorCode, FogMode, Material, Shader};

pub fn gamma_constant(value:f32)->f32 {
    if value>1.0 {value} else if value<0.0 {0.0} else if value>=0.95 {1.0} else {((value*255.0).round_ties_even()/255.0).powf(2.2)}
}

pub fn unlit_sprite_modulation(material:&Material)->Result<[f32;4],Error> {
    if material.shader!=Shader::UnlitGeneric {return Err(crate::error(ErrorCode::RootKind,None));}
    let mut parameters=material.first_parameters.clone();
    // CSprite and CGlowOverlay supply this per draw, rather than preserving a
    // previous user's HDR scale on the shared material.
    parameters.insert(b"$hdrcolorscale".to_vec(),b"1".to_vec());
    let color=crate::model::unlit_color_modulation(&parameters,material.selection_environment)?;
    Ok([color[0],color[1],color[2],1.0])
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum Orientation { ParallelUpright=0, FacingUpright=1, Parallel=2, Oriented=3, ParallelOriented=4 }

#[derive(Clone,Debug,PartialEq)]
pub struct SpriteMaterial {
    pub orientation:Orientation,
    pub origin:Option<[f32;2]>,
    pub srgb:bool,
    pub ignore_vertex_colors:bool,
    pub ignore_z:bool,
    pub no_cull:bool,
    pub no_fog:bool,
    pub color:[f32;3],
    pub alpha:f32,
    pub hdr_color_scale:f32,
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Pass {
    pub blend:BlendState,
    pub depth_test:bool,
    pub depth_write:bool,
    pub cull:CullState,
    pub fog:FogMode,
    pub vertex_color:bool,
    pub constant_color:bool,
    /// Sprite frame-blend mode supplies two independent texture frames/passes.
    pub frame_offset:u8,
}

impl SpriteMaterial {
    pub fn compile(material:&Material)->Result<Self,Error> {
        if material.shader!=Shader::Sprite || material.particle.is_some() {
            return Err(crate::error(ErrorCode::RootKind,None));
        }
        let p=&material.first_parameters;
        let orientation=match p.get(b"$spriteorientation".as_slice()).map(|value|value.to_ascii_lowercase()).as_deref() {
            Some(b"facing_upright")=>Orientation::FacingUpright,
            Some(b"vp_parallel")=>Orientation::Parallel,
            Some(b"oriented")=>Orientation::Oriented,
            Some(b"vp_parallel_oriented")=>Orientation::ParallelOriented,
            _=>Orientation::ParallelUpright,
        };
        let origin=match p.get(b"$spriteorigin".as_slice()) {
            Some(value) if value.starts_with(b"[")&&value.ends_with(b"]")=>{
                let values=std::str::from_utf8(&value[1..value.len()-1]).ok().and_then(|text|text.split_ascii_whitespace().map(str::parse::<f32>).collect::<Result<Vec<_>,_>>().ok())
                    .filter(|values|(2..=4).contains(&values.len())&&values.iter().all(|value|value.is_finite()))
                    .ok_or_else(||crate::error(ErrorCode::InvalidParameter,Some(b"$spriteorigin".to_vec())))?;
                Some([values[0],values[1]])
            }
            None=>Some([0.0,0.0]),
            _=>None,
        };
        Ok(Self {orientation,origin,srgb:crate::integer_or(p,b"$nosrgb",1)?==0,
            ignore_vertex_colors:crate::integer_or(p,b"$ignorevertexcolors",0)?!=0,
            ignore_z:material.features.ignore_z,no_cull:material.features.no_cull,no_fog:material.features.no_fog,
            color:crate::color_or(p,b"$color",[1.0;3])?,alpha:crate::float_or(p,b"$alpha",1.0)?,hdr_color_scale:crate::float_or(p,b"$hdrcolorscale",1.0)?})
    }

    /// [left, right, up, down], in authored mapping pixels, not VTF storage size.
    pub fn extents(&self,width:f32,height:f32)->[f32;4] {
        let origin=self.origin.unwrap_or([0.5,0.5]);
        [-width*origin[0],width*(1.0-origin[0]),height*origin[1],height*(origin[1]-1.0)]
    }

    pub fn passes(&self,mode:u8)->Result<Vec<Pass>,Error> {
        use BlendFactor::{One,Zero,SourceAlpha,OneMinusSourceAlpha};
        let blend=|enabled,source,destination|BlendState{enabled,equation:crate::BlendEquation::Add,source,destination};
        let mut pass=Pass{blend:blend(false,One,Zero),depth_test:!self.ignore_z,depth_write:!self.ignore_z,cull:CullState::None,fog:FogMode::Color,vertex_color:false,constant_color:false,frame_offset:0};
        match mode {
            0=>{},
            1|2|4|7=>{pass.blend=blend(true,SourceAlpha,OneMinusSourceAlpha);pass.depth_write=false;pass.vertex_color=true;},
            3|9=>{pass.blend=blend(true,SourceAlpha,One);pass.depth_test=false;pass.depth_write=false;pass.fog=FogMode::Black;pass.vertex_color=true;},
            5|8=>{pass.blend=blend(true,SourceAlpha,One);pass.depth_write=false;pass.fog=FogMode::Black;pass.vertex_color=!self.ignore_vertex_colors;pass.constant_color=true;},
            6|10=>return Ok(Vec::new()),
            _=>return Err(crate::error(ErrorCode::InvalidParameter,Some(b"$spriterendermode".to_vec()))),
        }
        let mut passes=vec![pass];
        if mode==7 {
            // Sprite_DX9 resets shadow state between these two distinct passes.
            pass.blend=blend(true,OneMinusSourceAlpha,One);pass.fog=FogMode::Black;pass.cull=if self.no_cull {CullState::None} else {CullState::Back};passes.push(pass);
        } else if mode==8 {pass.frame_offset=1;passes.push(pass);}
        if self.no_fog {for pass in &mut passes {pass.fog=FogMode::Disabled;}}
        Ok(passes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sprite()->SpriteMaterial { SpriteMaterial{orientation:Orientation::Parallel,origin:Some([0.5,0.5]),srgb:false,ignore_vertex_colors:false,ignore_z:false,no_cull:false,no_fog:false,color:[1.0;3],alpha:1.0,hdr_color_scale:1.0} }
    #[test]
    fn render_modes_keep_glow_depth_and_both_alpha_add_passes() {
        let sprite=sprite();let glow=sprite.passes(9).unwrap()[0];
        assert!(!glow.depth_test);assert!(!glow.depth_write);assert!(glow.vertex_color);assert_eq!(glow.fog,FogMode::Black);
        let alpha_add=sprite.passes(7).unwrap();assert_eq!(alpha_add.len(),2);
        assert_eq!(alpha_add[0].blend.destination,BlendFactor::OneMinusSourceAlpha);
        assert_eq!(alpha_add[1].blend.source,BlendFactor::OneMinusSourceAlpha);
        assert_eq!(alpha_add[1].blend.destination,BlendFactor::One);
        assert_eq!(alpha_add[0].cull,CullState::None);assert_eq!(alpha_add[1].cull,CullState::Back);
        assert!(sprite.passes(5).unwrap()[0].vertex_color);
        assert!(sprite.passes(6).unwrap().is_empty());assert!(sprite.passes(10).unwrap().is_empty());
    }
    #[test]
    fn origin_and_frame_blend_are_not_generic_centered_particle_quads() {
        let mut sprite=sprite();sprite.origin=Some([0.25,0.75]);
        assert_eq!(sprite.extents(128.0,64.0),[-32.0,96.0,48.0,-16.0]);
        let frames=sprite.passes(8).unwrap();assert_eq!(frames.len(),2);assert_eq!(frames[0].frame_offset,0);assert_eq!(frames[1].frame_offset,1);
    }

    #[test]
    fn shader_and_type_initializers_are_distinct_from_parameter_help_defaults() {
        let playsrc_vmt::Composition::Complete(document)=playsrc_vmt::compose(b"Sprite {}","materials/sprite.vmt",&[],&playsrc_keyvalues::ConditionEnvironment::default(),playsrc_vmt::Limits::default()).unwrap() else {panic!("unexpected dependency")};
        let material=crate::resolve_for_environment(&document,crate::SelectionEnvironment::default()).unwrap();
        let sprite=SpriteMaterial::compile(&material).unwrap();
        assert_eq!(sprite.origin,Some([0.0,0.0]));assert!(!sprite.ignore_vertex_colors);assert!(!sprite.srgb);
        assert_eq!(sprite.alpha,1.0);assert_eq!(sprite.hdr_color_scale,1.0);
    }
}
