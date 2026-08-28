//! Source client rope integration (CSimplePhysics and CBaseRopePhysics).
//! The caller owns endpoints, client-frame admission, lighting and random draws.

pub type Vector=[f32;3];
#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct Node {pub position:Vector,pub previous:Vector,pub predicted:Vector}

#[derive(Clone,Debug)]
pub struct Rope {
    pub nodes:Vec<Node>,
    spring:f32,
    predicted_time:f32,
    step:i32,
    endpoints:[Vector;2],
    previous_endpoints:[Vector;2],
    present:[bool;2],previous_present:[bool;2],
    locked:u8,
    previous_locked:u8,
    force_frames:i32,
    new_data:bool,
    no_wind:bool,
    wind:Vector,
    gust_time:f32,
    gust_lifetime:f32,
    next_gust:f32,
    impulse:Vector,
    previous_impulse:Vector,
}

pub struct Geometry {pub back:Option<playsrc_beam::Mesh>,pub solid:Option<playsrc_beam::Mesh>}
pub struct Draw<'a> {
    pub lighting:&'a [Vector],pub color_modulation:Vector,pub width:f32,pub subdivisions:usize,
    pub length:i32,pub slack:i32,pub texture_scale:f32,pub mapping_height:u32,
    pub view:&'a crate::pixel_visibility::View,pub screen_width:u32,pub samples:u32,pub has_back:bool,
}
pub fn geometry(rope:&Rope,draw:Draw<'_>)->Geometry{
    assert_eq!(draw.lighting.len(),rope.nodes.len());assert!(draw.mapping_height>0&&draw.screen_width>0&&draw.subdivisions<=7);
    let count=rope.nodes.len();let step=1.0/(draw.subdivisions+1) as f32;
    let mut segments=Vec::with_capacity((count-1)*(draw.subdivisions+1)+1);
    for index in 0..count{
        let color=std::array::from_fn::<_,3,_>(|axis|draw.lighting[index][axis]*draw.color_modulation[axis]);
        segments.push(playsrc_beam::Segment{position:rope.nodes[index].predicted,width:draw.width,color:[color[0],color[1],color[2],1.0],texture_coordinate:0.0});
        if index+1==count{continue;}
        let a=rope.nodes[index.saturating_sub(1)].predicted;let b=rope.nodes[index].predicted;
        let c=rope.nodes[index+1].predicted;let d=rope.nodes[(index+2).min(count-1)].predicted;
        let cubic=scale(add(add(add(scale(a,-1.0),scale(b,3.0)),scale(c,-3.0)),d),0.5);
        let quadratic=scale(sub(add(add(scale(a,2.0),scale(b,-5.0)),scale(c,4.0)),d),0.5);
        let linear=scale(add(scale(a,-1.0),c),0.5);
        let increment=std::array::from_fn::<_,3,_>(|axis|step*((draw.lighting[index+1][axis]-draw.lighting[index][axis])*draw.color_modulation[axis]));
        for division in 0..draw.subdivisions{
            let t=(division+1) as f32/(draw.subdivisions+1) as f32;let t2=t*t;let t3=t2*t;
            let position=add(add(add(b,scale(linear,t)),scale(quadratic,t2)),scale(cubic,t3));
            let previous=segments.last().unwrap().color;
            segments.push(playsrc_beam::Segment{position,width:draw.width,color:[previous[0]+increment[0],previous[1]+increment[1],previous[2]+increment[2],1.0],texture_coordinate:0.0});
        }
    }
    let total=(4.0/draw.texture_scale)*(draw.length as f32+draw.slack as f32-100.0);
    let points=(count-1)*draw.subdivisions+1;
    let increment=(total/points as f32)/draw.mapping_height as f32;
    let mut coordinate=0.0;for segment in &mut segments{segment.texture_coordinate=coordinate;coordinate+=increment;}
    if !draw.has_back||draw.samples>1{return Geometry{back:None,solid:Some(playsrc_beam::build(&segments,draw.view.origin))};}
    let half=draw.screen_width as f32/2.0;let mut solid_widths=Vec::with_capacity(segments.len());let mut maximum=0.0_f32;
    for segment in &mut segments{
        let depth=dot(draw.view.forward,sub(segment.position,draw.view.origin)).max(0.1);
        let screen_width=draw.width*half/depth;
        let solid_width=if screen_width<0.3{
            segment.color[3]=0.2;segment.width=0.3*depth/half;0.0
        }else{
            segment.color[3]=if screen_width>1.75{0.5}else{0.2+(0.5-0.2)*(screen_width-0.3)/(1.75-0.3)};
            (draw.width-depth*1.4/draw.screen_width as f32).max(0.0)
        };
        maximum=maximum.max(solid_width);solid_widths.push(solid_width);
    }
    let back=Some(playsrc_beam::build(&segments,draw.view.origin));
    let solid=if maximum<=0.3{None}else{
        for (segment,width) in segments.iter_mut().zip(solid_widths){segment.width=width;segment.color[3]=((width-0.3)/(1.0-0.3)).clamp(0.0,1.0);}
        Some(playsrc_beam::build(&segments,draw.view.origin))
    };
    Geometry{back,solid}
}

impl Rope {
    pub fn new(endpoints:[Vector;2],count:usize,length:i32,slack:i32,locked:u8,no_wind:bool,initial_hang:bool,random:&mut impl FnMut(f32,f32)->f32)->Self {
        assert!((2..=10).contains(&count));
        let nodes=(0..count).map(|index|{
            let position=lerp(endpoints[0],endpoints[1],index as f32/(count-1) as f32);
            Node{position,previous:position,predicted:position}
        }).collect();
        let mut rope=Self{nodes,spring:0.0,predicted_time:0.0,step:0,endpoints,previous_endpoints:endpoints,present:[true;2],previous_present:[true;2],locked,previous_locked:0,force_frames:0,new_data:true,no_wind,
            wind:[0.0;3],gust_time:0.0,gust_lifetime:0.0,next_gust:0.0,impulse:[0.0;3],previous_impulse:[0.0;3]};
        rope.set_length(length,slack);
        if initial_hang{rope.simulate(5.0,false,[0.0;3]);}
        rope.next_gust=random(1.0,3.0);
        rope
    }
    pub fn set_length(&mut self,length:i32,slack:i32){
        // The SDK expression divides integers before ResetSpringLength(float).
        self.spring=((length+slack-100)/(self.nodes.len() as i32-1)).max(0) as f32;
        self.new_data=true;
    }
    pub fn changed(&mut self){self.new_data=true;}
    pub fn set_endpoints(&mut self,endpoints:[Option<Vector>;2],locked:u8){
        for (index,point) in endpoints.into_iter().enumerate(){
            if self.present[index]!=point.is_some(){self.new_data=true;}
            self.present[index]=point.is_some();if let Some(point)=point{self.endpoints[index]=point;}
        }
        self.locked=locked;
    }
    pub fn shake(&mut self,center:Vector,radius:f32,magnitude:f32){
        for node in &self.nodes{let amount=1.0-length(sub(node.position,center))/radius;if amount>=0.0{self.impulse[2]+=amount*magnitude;}}
    }
    pub fn advance(&mut self,seconds:f32,view:Vector,environment_wind:Vector,random:&mut impl FnMut(f32,f32)->f32)->bool{
        assert!(seconds.is_finite()&&seconds>=0.0);
        let mut apply_wind=false;
        let resting=if self.previous_locked!=self.locked{
            self.force_frames=10;self.previous_locked=self.locked;false
        }else if self.new_data{false}
        else if self.endpoint_moved(0)||self.endpoint_moved(1){false}
        else{
            apply_wind=!self.no_wind&&distance_to_segment(view,self.nodes[0].position,self.nodes.last().unwrap().position)<1000.0;
            if self.previous_impulse!=self.impulse{self.previous_impulse=self.impulse;false}
            else{!self.points_moved()&&!apply_wind}
        };
        if resting{return false;}
        self.simulate(seconds,apply_wind,environment_wind);
        self.new_data=false;self.gust_time+=seconds;self.next_gust-=seconds;
        if self.next_gust<=0.0{
            self.wind=normalize([random(-1.0,1.0),random(-1.0,1.0),random(-1.0,1.0)]);
            self.wind=scale(self.wind,50.0);self.wind=scale(self.wind,random(-1.0,1.0));
            self.gust_time=0.0;self.gust_lifetime=random(2.0,3.0);self.next_gust=random(3.0,4.0);
        }
        true
    }
    fn endpoint_moved(&mut self,index:usize)->bool{
        if self.locked&(1<<index)==0{return false;}
        let valid=self.previous_present[index];self.previous_present[index]=self.present[index];
        let previous=self.previous_endpoints[index];if self.present[index]{self.previous_endpoints[index]=self.endpoints[index];}
        if !valid&&!self.present[index]{return true;}
        previous.iter().zip(self.endpoints[index]).any(|(a,b)|(a-b).abs()>0.1)
    }
    fn points_moved(&mut self)->bool{
        if self.nodes.iter().any(|node|dot(sub(node.position,node.previous),sub(node.position,node.previous))>0.03){return true;}
        self.force_frames-=1;self.force_frames>0
    }
    fn simulate(&mut self,seconds:f32,apply_wind:bool,environment_wind:Vector){
        const STEP:f32=1.0/50.0;
        self.predicted_time+=seconds;
        let next=(self.predicted_time/STEP).ceil() as i32;
        for _ in self.step..next{
            for node in &mut self.nodes{
                let mut acceleration=[0.0,0.0,-1500.0];
                if apply_wind{
                    if dot(environment_wind,environment_wind)>0.0{acceleration=add(acceleration,scale(environment_wind,10.0));}
                    else if self.gust_time<self.gust_lifetime{
                        let factor=(1.0-((self.gust_time/self.gust_lifetime) as f64*std::f64::consts::PI).cos()) as f32;
                        acceleration=add(acceleration,scale(self.wind,factor));
                    }
                }
                acceleration=add(acceleration,scale(self.impulse,20.0));self.impulse=scale(self.impulse,0.95);
                let previous=node.position;
                node.position=add(add(node.position,scale(sub(node.position,node.previous),0.98)),scale(acceleration,STEP*STEP*0.5));
                node.previous=previous;
            }
            for _ in 0..3{
                for index in 0..self.nodes.len()-1{
                    let mut delta=sub(self.nodes[index].position,self.nodes[index+1].position);
                    let squared=dot(delta,delta);
                    if squared>self.spring*self.spring{
                        delta=scale(delta,1.0-self.spring/squared.sqrt());
                        self.nodes[index].position=sub(self.nodes[index].position,scale(delta,0.5));
                        self.nodes[index+1].position=add(self.nodes[index+1].position,scale(delta,0.5));
                    }
                }
                if self.locked&1!=0&&self.present[0]{self.nodes[0].position=self.endpoints[0];}
                if self.locked&2!=0&&self.present[1]{self.nodes.last_mut().unwrap().position=self.endpoints[1];}
            }
        }
        self.step=next;
        let fraction=(self.predicted_time-(self.step as f32*STEP-STEP))/STEP;
        for node in &mut self.nodes{node.predicted=lerp(node.previous,node.position,fraction);}
    }
}

fn add(a:Vector,b:Vector)->Vector{std::array::from_fn(|i|a[i]+b[i])}
fn sub(a:Vector,b:Vector)->Vector{std::array::from_fn(|i|a[i]-b[i])}
fn scale(a:Vector,b:f32)->Vector{a.map(|v|v*b)}
fn dot(a:Vector,b:Vector)->f32{a[0]*b[0]+a[1]*b[1]+a[2]*b[2]}
fn length(a:Vector)->f32{dot(a,a).sqrt()}
fn normalize(a:Vector)->Vector{let n=length(a);if n==0.0{a}else{scale(a,1.0/n)}}
fn lerp(a:Vector,b:Vector,t:f32)->Vector{add(a,scale(sub(b,a),t))}
fn distance_to_segment(p:Vector,a:Vector,b:Vector)->f32{
    let delta=sub(b,a);let squared=dot(delta,delta);let t=if squared==0.0{0.0}else{(dot(sub(p,a),delta)/squared).clamp(0.0,1.0)};
    length(sub(p,add(a,scale(delta,t))))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn fifty_hertz_prediction_and_locked_endpoints_do_not_depend_on_render_rate(){
        let mut random=|low:f32,high:f32|(low+high)*0.5;
        let mut rope=Rope::new([[0.0;3],[200.0,0.0,0.0]],5,200,120,3,true,false,&mut random);
        let mut split=rope.clone();
        rope.advance(0.02,[0.0;3],[0.0;3],&mut random);
        split.advance(0.01,[0.0;3],[0.0;3],&mut random);split.advance(0.01,[0.0;3],[0.0;3],&mut random);
        assert_eq!(rope.nodes,split.nodes);assert_eq!(rope.nodes[0].position,[0.0;3]);assert_eq!(rope.nodes[4].position,[200.0,0.0,0.0]);
        assert!((rope.nodes[2].position[2]+0.3).abs()<1e-6);
    }
    #[test]
    fn initial_hang_uses_source_slack_fudge_and_integer_spring_length(){
        let rope=Rope::new([[0.0;3],[203.0,0.0,0.0]],5,203,120,3,true,true,&mut |a,b|(a+b)*0.5);
        assert_eq!(rope.spring,55.0);assert!(rope.nodes[2].predicted[2]< -20.0);
        assert_eq!(rope.step,250);
    }
    #[test]
    fn cable_antialiasing_uses_the_authored_back_material_only_without_hardware_samples(){
        let rope=Rope::new([[0.0;3],[203.0,0.0,0.0]],5,203,120,3,true,true,&mut |a,b|(a+b)*0.5);
        let view=crate::pixel_visibility::View::perspective([-10000.0,0.0,0.0],0.0,0.0,75.0,16.0/9.0,1.0,30000.0,720);
        let lighting=vec![[0.5;3];5];
        let draw=|samples|Draw{lighting:&lighting,color_modulation:[1.0;3],width:0.5,subdivisions:2,length:203,slack:120,texture_scale:1.0,mapping_height:16,view:&view,screen_width:1280,samples,has_back:true};
        let soft=geometry(&rope,draw(1));assert!(soft.solid.is_none());let back=soft.back.unwrap();assert_eq!(back.vertices.len(),26);assert_eq!(back.vertices[0].color[3],51);
        let hardware=geometry(&rope,draw(4));assert!(hardware.back.is_none());let solid=hardware.solid.unwrap();assert_eq!(solid.vertices.len(),26);assert_eq!(solid.indices.len(),72);assert_eq!(solid.vertices[0].color[3],255);
    }
}
