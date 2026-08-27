//! CSprite map-entity animation and input state. Material frame counts are
//! supplied by the configured model/material owner before simulation begins.
use std::collections::BTreeMap;
use crate::{EntityHandle, EntityWorld, ExternalClassBinding, Variant, source_integer};

pub fn is_sprite(class: &[u8]) -> bool {
    [b"env_sprite".as_slice(),b"env_sprite_oriented",b"env_glow"].iter().any(|value|class.eq_ignore_ascii_case(value))
}

pub(crate) fn spawn_angles(class: &[u8], mut angles: [f32;3]) -> [f32;3] {
    if class.eq_ignore_ascii_case(b"env_sprite_oriented") {
        angles[1]=(360.0/65536.0)*((((angles[1]+180.0)*(65536.0/360.0)) as i32 & 65535) as f32);
    } else if is_sprite(class) && angles[1]!=0.0 && angles[2]==0.0 {
        angles[2]=angles[1];angles[1]=0.0;
    }
    angles
}

pub fn bindings() -> impl Iterator<Item=ExternalClassBinding> {
    [b"env_sprite".as_slice(),b"env_sprite_oriented",b"env_glow"].into_iter().map(|class|ExternalClassBinding {
        classname:class.to_vec(), inputs:[b"SetScale".as_slice(),b"HideSprite",b"ShowSprite",b"ToggleSprite",b"ColorRedValue",b"ColorGreenValue",b"ColorBlueValue"].into_iter().map(<[u8]>::to_vec).collect(),
    })
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Presentation { pub frame:f32, pub scale:f32, pub brightness:u8, pub active:bool }

#[derive(Clone,Debug)]
struct Sprite {
    entity:EntityHandle,
    state:Presentation,
    max_frame:f32,
    frame_rate:f32,
    once:bool,
    animate:bool,
    last_time:f32,
}

impl Sprite {
    fn turn_on(&mut self,now:f32) {
        self.state.active=true;self.state.frame=0.0;
        self.animate=(self.frame_rate!=0.0&&self.max_frame>1.0)||self.once;
        if self.animate { self.last_time=now; }
    }
    fn turn_off(&mut self) { self.state.active=false;self.animate=false; }
    fn advance(&mut self,now:f32) {
        if !self.animate { return; }
        self.state.frame+=self.frame_rate*(now-self.last_time);
        if self.state.frame>self.max_frame {
            if self.once { self.turn_off(); }
            else if self.max_frame>0.0 { self.state.frame%=self.max_frame; }
        }
        self.last_time=now;
    }
}

#[derive(Clone,Debug,Default)]
pub struct Sprites { entries:BTreeMap<u32,Sprite> }

impl Sprites {
    pub fn from_world(world:&EntityWorld,now:f32,mut frames:impl FnMut(&[u8])->Option<u32>)->Result<Self,u32> {
        let mut result=Self::default();
        for entity in world.live_handles() {
            let runtime=world.entity(entity).expect("live sprite entity");
            if !is_sprite(&runtime.classname) { continue; }
            let source=runtime.source_index as u32;
            let value=|name:&[u8]|runtime.definition.pairs.iter().find(|pair|pair.key.eq_ignore_ascii_case(name)).map(|pair|pair.value.as_slice()).unwrap_or_default();
            let number=|name|Variant::String(value(name).to_vec()).as_float().ok_or(source);
            let count=frames(value(b"model")).filter(|count|*count>0).ok_or(source)?;
            let flags=source_integer(value(b"spawnflags"));
            let mut sprite=Sprite {entity,state:Presentation{frame:0.0,scale:number(b"scale")?.clamp(0.0,64.0),brightness:runtime.render.color[3],active:false},
                max_frame:(count-1) as f32,frame_rate:number(b"framerate")?,once:flags&2!=0,animate:false,last_time:now};
            if runtime.targetname.is_none() || flags&1!=0 { sprite.turn_on(now); }
            result.entries.insert(source,sprite);
        }
        Ok(result)
    }

    pub fn advance(&mut self,now:f32) { for sprite in self.entries.values_mut() { sprite.advance(now); } }
    pub fn presentation(&self,world:&EntityWorld,source:u32)->Option<Presentation> {
        let sprite=self.entries.get(&source)?;
        world.entity(sprite.entity).map(|_|sprite.state)
    }

    /// Channel inputs return the new base-entity render color to the world owner;
    /// they do not introduce a second authority for inherited Color/Alpha inputs.
    pub fn input(&mut self,world:&EntityWorld,entity:EntityHandle,input:&[u8],value:&Variant,now:f32)->Option<[u8;4]> {
        let runtime=world.entity(entity)?;
        let sprite=self.entries.get_mut(&(runtime.source_index as u32))?;
        if sprite.entity!=entity { return None; }
        if input.eq_ignore_ascii_case(b"ShowSprite") { sprite.turn_on(now); }
        else if input.eq_ignore_ascii_case(b"HideSprite") { sprite.turn_off(); }
        else if input.eq_ignore_ascii_case(b"ToggleSprite") { if sprite.state.active {sprite.turn_off();} else {sprite.turn_on(now);} }
        else if input.eq_ignore_ascii_case(b"SetScale") { if let Some(value)=value.as_float() {sprite.state.scale=value;} }
        else if let Some(channel)=[b"ColorRedValue".as_slice(),b"ColorGreenValue",b"ColorBlueValue"].iter().position(|name|input.eq_ignore_ascii_case(name)) {
            let value=value.as_float()?;
            let rounded=((value+12_582_912.0).to_bits()&0x7f_ffff) as i32-0x40_0000;
            let mut color=runtime.render.color;color[channel]=rounded.clamp(0,255) as u8;return Some(color);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse,Limits,EntityWorldConfig};
    fn world(text:&[u8])->EntityWorld { EntityWorld::compile(&parse(text,Limits::default()).unwrap(),EntityWorldConfig{external_classes:bindings().collect(),..Default::default()}).unwrap().0 }
    #[test]
    fn spawn_angles_and_max_frame_modulo_follow_sprite_not_texture_frame_count() {
        let world=world(br#"{"classname" "env_sprite" "model" "sprites/test.vmt" "angles" "0 90 0" "framerate" "10" "scale" "100"}"#);
        let handle=world.live_handles()[0];
        assert_eq!(world.entity(handle).unwrap().world_transform.angles,[0.0,0.0,90.0]);
        let mut sprites=Sprites::from_world(&world,0.0,|_|Some(3)).unwrap();
        assert_eq!(sprites.presentation(&world,0).unwrap().scale,64.0);
        sprites.advance(0.2);assert_eq!(sprites.presentation(&world,0).unwrap().frame,2.0);
        sprites.advance(0.3);assert!((sprites.presentation(&world,0).unwrap().frame-1.0).abs()<1e-6);
        sprites.input(&world,handle,b"ShowSprite",&Variant::Void,0.3);
        assert_eq!(sprites.presentation(&world,0).unwrap().frame,0.0);
        sprites.input(&world,handle,b"SetScale",&Variant::float(-2.0),0.3);
        assert_eq!(sprites.presentation(&world,0).unwrap().scale,-2.0);
    }
    #[test]
    fn ordinary_two_frame_sprites_do_not_think_but_once_sprites_turn_off() {
        let world=world(br#"{"classname" "env_sprite" "model" "sprites/test.vmt" "framerate" "10"}
          {"classname" "env_sprite_oriented" "model" "sprites/test.vmt" "targetname" "once" "spawnflags" "2" "framerate" "10" "angles" "0 90 10"}"#);
        let mut sprites=Sprites::from_world(&world,0.0,|_|Some(2)).unwrap();
        sprites.advance(1.0);assert_eq!(sprites.presentation(&world,0).unwrap().frame,0.0);
        assert!(!sprites.presentation(&world,1).unwrap().active);
        let handle=world.live_handles()[1];assert_eq!(world.entity(handle).unwrap().world_transform.angles,[0.0,270.0,10.0]);
        sprites.input(&world,handle,b"ShowSprite",&Variant::Void,1.0);sprites.advance(1.2);
        assert!(!sprites.presentation(&world,1).unwrap().active);
        assert!(Sprites::from_world(&world,0.0,|_|None).is_err());
    }
}
