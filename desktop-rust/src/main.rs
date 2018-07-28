extern crate byteorder;
///Used to get screen resolution.
extern crate screenshot;
extern crate inputbot;
extern crate ron;
extern crate bincode;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use prelude::*;
use std::{
  net::{TcpStream},
  io::{self,BufRead,BufReader},
  fs::{File},
  cmp::{Ordering},
  env,
  ops::{self,RangeInclusive},
  path::{Path},
};
use byteorder::{NetworkEndian,ByteOrder,ReadBytesExt};
use inputbot::{MouseCursor};
use rect::*;
use network::{Remote};
use absm::{AbsmSession,ServerInfo};

mod prelude {
  pub use std::error::Error as ErrorTrait;
  pub type Error = Box<ErrorTrait>;
  pub type Result<T> = ::std::result::Result<T,Error>;
  
  pub use std::{
    io::{Write,Read},
    fmt,mem,
  };
  pub use serde::{Serialize,Deserialize};
  pub use network::{Connection,LocalBuffer,NetBuffer};
  
  pub enum Never {}
  impl Never {
    fn as_never(&self)->! {unsafe{::std::hint::unreachable_unchecked()}}
  }
  impl fmt::Display for Never {
    fn fmt(&self,_: &mut fmt::Formatter)->fmt::Result {self.as_never()}
  }
  impl fmt::Debug for Never {
    fn fmt(&self,_: &mut fmt::Formatter)->fmt::Result {self.as_never()}
  }
  impl ErrorTrait for Never {}
}

#[macro_use]
mod rect;
mod network;
mod absm;

pub struct Setup {
  ///Map from input device coordinates to output client coordinates.
  pub mapping: Mapping,
  ///Specify a minimum and a maximum on the final client coordinates.
  pub clip: Rect<i32>,
  ///Specify a range of pressures.
  ///Events with a pressure outside this range are ignored.
  pub pressure: [f32; 2],
  ///Specify a range of sizes, similarly to `pressure`.
  pub size: [f32; 2],
}
impl Setup {
  fn new(info: &ServerInfo,config: &Config)->Setup {
    //Target area is set immutably by the config
    let target=config.target;
    //Start off with source area as the entire device screen
    //Source area is more mutable than target area
    let mut source=Rect{min: Pair([0.0; 2]),max: info.server_screen_res};
    println!("device screen area: {}",source);
    
    //Correct any device rotations
    if config.correct_device_orientation {
      if source.aspect()!=target.aspect() {
        //Source screen should be rotated 90° counterclockwise to correct orientation
        source.rotate_negative();
        println!("rotated 90° counterclockwise to correct device orientation");
      }else{
        println!("device orientation is aligned with client orientation");
      }
    }else{
      println!("device orientation correction is disabled");
    }
    
    //Apply config device source area proportions
    let mut source=Rect{
      min: source.map(|int| int as f32).denormalizer().apply(config.source.min),
      max: source.map(|int| int as f32).denormalizer().apply(config.source.max),
    };
    
    //Correct orientation if source and target don't have matching aspects
    if config.correct_orientation {
      if source.aspect()!=target.aspect() {
        source.rotate_negative();
        println!("rotated 90° counterclockwise to correct orientation mismatch");
      }else{
        println!("final orientation matches target orientation");
      }
    }else{
      println!("final orientation correction is disabled");
    }
    
    //Shrink a source axis to match target aspect ratio
    if config.keep_aspect_ratio {
      let shrink=|source: &mut Rect<f32>,shrink_axis: Axis| {
        let fixed_axis=shrink_axis.swap();
        //Get the target size of the shrink axis
        let target=target.virtual_size(shrink_axis) as f32*source.virtual_size(fixed_axis)
                            / target.virtual_size(fixed_axis) as f32;
        source.resize_virtual_axis(shrink_axis,target);
      };
      match target.map(|int| int as f32).aspect_ratio().partial_cmp(&source.aspect_ratio()).unwrap() {
        Ordering::Greater=>{
          //Shrink vertically to match aspect ratio
          let old=source.virtual_size(Axis::Y);
          shrink(&mut source,Axis::Y);
          println!(
            "shrank source area vertically from {} to {} to match target aspect ratio",
            old,source.virtual_size(Axis::Y)
          );
        },
        Ordering::Less=>{
          //Shrink horizontally to match aspect ratio
          let old=source.virtual_size(Axis::X);
          shrink(&mut source,Axis::X);
          println!(
            "shrank source area horizontally from {} to {} to match target aspect ratio",
            old,source.virtual_size(Axis::X)
          );
        },
        Ordering::Equal=>{
          println!("source aspect ratio matches target aspect ratio");
        },
      }
    }else{
      println!("aspect ratio correction is disabled");
    }
    
    println!("mapping source area {} to target area {}",source,target);
    
    let pressure=[
      config.pressure_range[0].unwrap_or(-std::f32::INFINITY),
      config.pressure_range[1].unwrap_or(std::f32::INFINITY),
    ];
    let size=[
      config.size_range[0].unwrap_or(-std::f32::INFINITY),
      config.size_range[1].unwrap_or(std::f32::INFINITY),
    ];
    
    println!("clipping target to {}",config.clip);
    println!("only allowing touches with pressures inside {:?} and sizes inside {:?}",pressure,size);
    
    Setup{
      mapping: source.normalizer().chain(&target.map(|int| int as f32).denormalizer()),
      clip: config.clip,
      pressure,size,
    }
  }
  
  fn consume(&mut self,ev: MouseMove) {
    if ev.pressure<self.pressure[0] || ev.pressure>self.pressure[1] {return}
    if ev.size<self.size[0] || ev.size>self.size[1] {return}
    let pos=self.mapping.apply(ev.pos);
    let adjusted=pair!(i=> (pos[i] as i32).max(self.clip.min[i]).min(self.clip.max[i]));
    MouseCursor.move_abs(adjusted[Axis::X],adjusted[Axis::Y]);
  }
}

#[derive(Deserialize,Serialize)]
#[serde(default)]
pub struct Config {
  ///The target area to be mapped, in screen pixels.
  pub target: Rect<i32>,
  ///The source area to be mapped, in normalized coordinates from `0.0` to `1.0`.
  pub source: Rect<f32>,
  ///After all transformations, clip mouse positions to this rectangle.
  pub clip: Rect<i32>,
  ///If the device screen is rotated, rotate it back to compensate.
  pub correct_device_orientation: bool,
  ///If after all transformations the source area is rotated, rotate it back to match target
  ///orientation (landscape or portrait).
  pub correct_orientation: bool,
  ///If the source area does not have the same aspect ratio as the target area, shrink it a bit
  ///in a single axis to fit.
  pub keep_aspect_ratio: bool,
  ///Only allow touches within this pressure range to go through.
  pub pressure_range: [Option<f32>; 2],
  ///Only allow touches within this size range to go through.
  pub size_range: [Option<f32>; 2],
  ///Connect to this remote.
  pub remote: Remote,
  ///When ADB port forwarding, map this port on the device.
  pub android_usb_port: u16,
  ///Whether to attempt to do ADB port forwarding automatically.
  ///The android device needs to have `USB Debugging` enabled.
  pub android_attempt_usb_connection: bool,
}
impl Default for Config {
  fn default()->Config {
    let screen_res=get_screen_resolution();
    Config{
      target: Rect{min: pair!(_=>0),max: screen_res},
      source: Rect{min: pair!(_=>0.05),max: pair!(_=>0.95)},
      clip: Rect{min: pair!(_=>0),max: screen_res},
      correct_device_orientation: true,
      correct_orientation: true,
      keep_aspect_ratio: true,
      pressure_range: [None; 2],
      size_range: [None; 2],
      remote: Remote::Tcp("localhost".into(),8517),
      android_usb_port: 8517,
      android_attempt_usb_connection: true,
    }
  }
}
impl Config {
  fn load_path(cfg_path: &str)->Config {
    println!("loading config file at '{}'",cfg_path);
    match File::open(&cfg_path) {
      Err(err)=>{
        println!("failed to open config at '{}', using defaults:\n {}",cfg_path,err);
        let config=Config::default();
        match File::create(&cfg_path) {
          Err(err)=>{
            println!("failed to create config file on '{}':\n {}",cfg_path,err);
          },
          Ok(mut file)=>{
            let cfg=ron::ser::to_string_pretty(&config,Default::default()).expect("error serializing default config");
            file.write_all(cfg.as_bytes()).expect("failed to write config file");
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
    }
  }
}

#[derive(Deserialize)]
struct MouseMove {
  pos: Pair<f32>,
  pressure: f32,
  size: f32,
}

fn get_screen_resolution()->Pair<i32> {
  let screenshot=screenshot::get_screenshot(0).expect("failed to get screen dimensions");
  Pair([screenshot.width() as i32,screenshot.height() as i32])
}

fn try_adb_forward<P: AsRef<Path>>(path: P,config: &Config)->Result<()> {
  use std::process::{Command};
  
  let local_port=match config.remote {
    Remote::Tcp(_,port)=>port,
    _ => {
      println!("not connecting through tcp, skipping adb port forwarding");
      return Ok(())
    },
  };
  println!("attempting to adb port forward using executable on '{}'",path.as_ref().display());
  let out=Command::new(path.as_ref())
    .arg("forward")
    .arg(format!("tcp:{}",local_port))
    .arg(format!("tcp:{}",config.android_usb_port))
    .output();
  match out {
    Ok(out)=>{
      if out.status.success() {
        println!(" adb exit code indicates success");
        Ok(())
      }else{
        println!(" adb exited with error exit code {:?}",out.status.code());
        let lines=|out| for line in String::from_utf8_lossy(out).trim().lines() {
          println!("  {}",line.trim());
        };
        println!(" adb output:");
        lines(&out.stdout);
        println!(" adb error output:");
        lines(&out.stderr);
        println!(" device might be disconnected or usb debugging disabled");
        Err("error exit code".into())
      }
    },
    Err(err)=>{
      println!(
        " failed to run command: {}",
        err
      );
      Err("failed to run command".into())
    },
  }
}

fn main() {
  //Parse arguments
  let exec_path;
  let cfg_path;
  {
    let mut args=env::args();
    exec_path=args.next().expect("first argument should always be executable path!");
    cfg_path=args.next().unwrap_or_else(|| String::from("config.txt"));
  }
  
  //Load configuration
  let config=Config::load_path(&cfg_path);
  
  //Try port forwarding using adb
  if config.android_attempt_usb_connection {
    let ok=try_adb_forward(&Path::new(&exec_path).with_file_name("adb"),&config)
      .or_else(|_err| try_adb_forward("adb",&config));
    match ok {
      Ok(())=>println!(
        "opened communication tunnel to android device"
      ),
      Err(_err)=>println!(
        "failed to open communication to android device, is USB Debugging enabled?"
      ),
    }
  }else{
    println!("usb android device connection is disabled");
  }
  
  let session=AbsmSession::new(config);
  
  loop {
    
  }
  
  /*
  //Create tcp stream to device
  //Tcp is used instead of udp because adb can only forward tcp ports
  println!("connecting to device at {}:{}...",config.host,config.port);
  let mut conn=TcpStream::connect((&*config.host,config.port)).expect("failed to connect to server");
  conn.set_nodelay(true).expect("failed to enable nodelay");
  conn.set_read_timeout(None).expect("failed to set timeout");
  conn.set_nonblocking(false).expect("failed to set nonblocking");
  println!("connected");
  
  let mut bincode_cfg=bincode::config();
  bincode_cfg.big_endian();
  loop {
    let mut msg_type=[0; 4];
    conn.read_exact(&mut msg_type).expect("failed to receive message");
    match &msg_type {
      b""=>{
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
  */
}
