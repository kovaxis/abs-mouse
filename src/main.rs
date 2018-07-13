extern crate byteorder;
extern crate screenshot;
extern crate inputbot;
extern crate ron;
extern crate bincode;
#[macro_use]
extern crate serde;

use std::{
  net::{TcpStream},
  io::{self,Write,Read,BufRead,BufReader},
  fs::{File},
  cmp::{Ordering},
  env,mem,
};
use byteorder::{NetworkEndian,ByteOrder,ReadBytesExt};
use inputbot::{MouseCursor};

macro_rules! pair {
  ($idx:ident=> $($tt:tt)*)=>{
    [{
      let $idx=0;
      $($tt)*
    },{
      let $idx=1;
      $($tt)*
    }]
  };
}

struct Setup {
  swap: bool,
  multiplier: [f32; 2],
  offset: [f32; 2],
  clip: Rect<i32>,
}
impl Setup {
  fn new(info: SetupInfo,config: &Config)->Setup {
    let target=config.target;
    let mut source=Rect{
      min: pair!(i=> config.source.min[i]*info.server_screen_res[i] as f32),
      max: pair!(i=> config.source.max[i]*info.server_screen_res[i] as f32),
    };
    
    println!("server screen resolution is {}x{}",info.server_screen_res[0],info.server_screen_res[1]);
    
    //Dimensions of the client target area, in pixels
    let target_dim=pair!(i=> target.max[i]-target.min[i]);
    //Dimensions of the server source area, in pixels
    let mut source_dim=pair!(i=> source.max[i]-source.min[i]);
    
    //Determine whether we want to swap source coordinates or not
    let swap=if config.correct_orientation {
      let client_cmp=target_dim[0].cmp(&target_dim[1]);
      let server_cmp=source_dim[0].partial_cmp(&source_dim[1]).unwrap();
      client_cmp!=Ordering::Equal && server_cmp!=Ordering::Equal && client_cmp!=server_cmp
    }else{
      false
    };
    //Rotate by 90 degrees counterclockwise if needed
    if swap {
      //Swap X and Y components
      source.min.swap(0,1);
      source.max.swap(0,1);
      //Negate X component
      mem::swap(&mut source.min[0],&mut source.max[0]);
      //Recalculate source dimensions
      source_dim=pair!(i=> source.max[i]-source.min[i]);
      println!("swapped source axis to correct orientation");
    }
    
    //How much offset to apply to the coordinates before scaling
    let mut pre_offset=pair!(i=> -source.min[i]);
    //How much offset to apply to the coordinates after scaling
    let post_offset=target.min;
    
    //Correct source area to keep aspect ratio
    if config.keep_aspect_ratio {
      //Shrink a given axis to a target size
      let shrink=|pre_offset: &mut f32,source_dim: &mut f32,target| {
        let distance=target-*source_dim;
        *pre_offset-=distance/2.0;
        *source_dim+=distance;
      };
      
      //Get server/client ratios to determine "bottleneck", or "dominant axis"
      let ratios=pair!(i=> source_dim[i]/target_dim[i] as f32);
      match ratios[0].partial_cmp(&ratios[1]) {
        Some(Ordering::Less)=>{
          //Shrink vertically to keep aspect ratio
          let target=source_dim[0]*target_dim[1] as f32/target_dim[0] as f32;
          println!("shrinking source area vertically from {}px to {}px to keep aspect ratio",source_dim[1],target);
          shrink(&mut pre_offset[1],&mut source_dim[1],target);
        },
        Some(Ordering::Greater)=>{
          //Shrink horizontally to keep aspect ratio
          let target=source_dim[1]*target_dim[0] as f32/target_dim[1] as f32;
          println!("shrinking source area horizontally from {}px to {}px to keep aspect ratio",source_dim[0],target);
          shrink(&mut pre_offset[0],&mut source_dim[0],target);
        },
        _ => {
          println!("aspect ratios match");
        },
      }
    }
    
    println!("mapping source area {:?} to target area {:?}",source,target);
    
    //Boil down factors into a multiplier and an offset
    //Picture `(x+pre_offset)*multiplier+post_offset`, simplified into
    //`x*multiplier+(pre_offset*multiplier+post_offset)`.
    let multiplier=pair!(i=> target_dim[i] as f32/source_dim[i] as f32);
    let offset=pair!(i=> pre_offset[i]*multiplier[i]+post_offset[i] as f32);
    
    Setup{
      multiplier,offset,swap,
      clip: config.clip,
    }
  }
  
  fn consume(&mut self,mut ev: MouseMove) {
    if self.swap {
      ev.pos.swap(0,1);
    }
    let adjusted=pair!(i=>{
      let adjusted=(ev.pos[i]*self.multiplier[i]+self.offset[i]) as i32;
      adjusted.max(self.clip.min[i]).min(self.clip.max[i])
    });
    MouseCursor.move_abs(adjusted[0],adjusted[1]);
  }
}

#[derive(Deserialize,Serialize,Debug)]
struct SetupInfo {
  server_screen_res: [i32; 2],
}
impl SetupInfo {
  fn build(self,config: &Config)->Setup {
    Setup::new(self,config)
  }
}

#[derive(Serialize,Deserialize,Copy,Clone,Debug)]
struct Rect<T> {
  min: [T; 2],
  max: [T; 2],
}

#[derive(Deserialize,Serialize)]
#[serde(default)]
struct Config {
  target: Rect<i32>,
  source: Rect<f32>,
  clip: Rect<i32>,
  correct_orientation: bool,
  keep_aspect_ratio: bool,
  host: String,
  port: u16,
}
impl Default for Config {
  fn default()->Config {
    let screen_res=get_screen_resolution();
    Config{
      target: Rect{min: [0; 2],max: screen_res},
      source: Rect{min: [0.0; 2],max: [1.0; 2]},
      clip: Rect{min: [0; 2],max: screen_res},
      correct_orientation: true,
      keep_aspect_ratio: false,
      host: String::from("localhost"),
      port: 8517,
    }
  }
}

#[derive(Deserialize)]
struct MouseMove {
  pos: [f32; 2],
  pressure: f32,
  size: f32,
}

fn get_screen_resolution()->[i32; 2] {
  let screenshot=screenshot::get_screenshot(0).expect("failed to get screen dimensions");
  [screenshot.width() as i32,screenshot.height() as i32]
}

fn main() {
  //Parse arguments
  let cfg_path;
  {
    let mut args=env::args().skip(1);
    cfg_path=args.next().unwrap_or_else(|| String::from("config.txt"));
  }
  
  //Load configuration
  let config=match File::open(&cfg_path) {
    Err(err)=>{
      println!("failed to open config at '{}', using defaults:\n {}",cfg_path,err);
      let config=Config::default();
      match File::create(&cfg_path) {
        Err(err)=>{
          println!("failed to create config file on '{}':\n {}",cfg_path,err);
        },
        Ok(mut file)=>{
          let cfg=ron::ser::to_string_pretty(&config,Default::default()).expect("failed to serialize config");
          file.write(cfg.as_bytes()).expect("error writing to config file");
          println!("created default config file on '{}'",cfg_path);
        },
      }
      config
    },
    Ok(file)=>{
      let config=ron::de::from_reader(file).expect("malformed configuration file");
      println!("loaded config file '{}'",cfg_path);
      config
    },
  };
  
  //Setup for the specific device
  let mut setup: Option<Setup>=None;
  
  let mut conn=TcpStream::connect((&*config.host,config.port)).expect("failed to connect to server");
  conn.set_nodelay(true).expect("failed to enable nodelay");
  conn.set_read_timeout(None).expect("failed to set timeout");
  conn.set_nonblocking(false).expect("failed to set nonblocking");
  println!("connected");
  
  let mut bincode_cfg=bincode::config();
  bincode_cfg.big_endian();
  loop {
    let mut msg_type=[0; 1];
    conn.read_exact(&mut msg_type).expect("failed to receive message");
    match msg_type {
      [0xDE]=>{
        //Mousemove message
        let mut data=[0; 16];
        conn.read_exact(&mut data).expect("failed to read message data");
        let mousemove=bincode_cfg.deserialize(&data).expect("malformed mousemove message");
        if let Some(ref mut setup) = setup {
          setup.consume(mousemove);
        }else{
          println!("failed to process mousemove, not setup yet!");
        }
      },
      [0xAD]=>{
        //Setup message
        let mut data=[0; 8];
        conn.read_exact(&mut data).expect("failed to read setup data");
        let info=bincode_cfg.deserialize(&data).expect("malformed setup message");
        setup=Some(Setup::new(info,&config));
      },
      [ty]=>println!("invalid message type {:x}",ty),
    }
  }
}
