use prelude::*;
use {Config,Setup};
use rect::*;
use network;

pub const ABSM_VERSION: (u16,u16)=(1,0);

thread_local!{
  static NET_BUFFER: NetBuffer=Default::default();
}

pub struct AbsmSession {
  config: Config,
  server_info: ServerInfo,
  setup: Setup,
  connection: Box<Connection>,
}
impl AbsmSession {
  ///Create an `AbsmSession` from the given configuration.
  pub fn new(config: Config)->AbsmSession {
    //Create connection
    println!("connecting to device at {}...",config.remote);
    let mut conn=config.remote.connect().expect("failed to connect to server");
    
    //Send open message
    println!("sending handshake-open message");
    NET_BUFFER.borrow(|buf| {
      buf.extend_from_slice(b"absM");
      network::encode_into(&mut *buf,&ABSM_VERSION).unwrap();
      {
        let mut header=|key,val| encode_header(&mut *buf,key,val);
        header(b"client_name",b"desktop-rust");
        header(b"frame_delay",&network::f32_to_bytes(0.25))
      }
      conn.send(&buf).expect("failed to send handshake-open");
    });
    
    //Receive server info
    println!("waiting for server-info reply");
    let server_info=NET_BUFFER.borrow(|buf| {
      conn.recv(&mut *buf).expect("failed to receive server-info");
      assert!(buf.len()>=8,"server-info message too short");
      assert!(&buf[0..4]==b"sInf","invalid server-info message");
      let remote_version: (u16,u16)=network::decode_from(&buf[4..8]).unwrap();
      assert!(
        ABSM_VERSION.0==remote_version.0,
        "abs-m protocol version mismatch: local {}.{} != remote {}.{}",
        ABSM_VERSION.0,ABSM_VERSION.1 , remote_version.0,remote_version.1
      );
      //Everything is ok, now decode server info headers
      println!("decoding server-info header fields");
      ServerInfo::from_decoder(HeaderDecoder(&buf[8..]))
    });
    
    //Create setup and notify to server
    println!("building setup");
    let setup=server_info.build(&config);
    println!("sending setup to server");
    NET_BUFFER.borrow(|buf| {
      buf.extend_from_slice(b"setp");
      {
        let mut header=|key,val| encode_header(&mut *buf,key,val);
        //header();
      }
      conn.send(buf).expect("failed to send setup information");
    });
    
    //Create state
    AbsmSession{config,server_info,setup,connection: conn}
  }
}

#[derive(Deserialize,Serialize,Debug)]
pub struct ServerInfo {
  pub server_screen_res: Pair<f32>,
}
impl ServerInfo {
  fn extend_from<'a>(&mut self,headers: HeaderDecoder<'a>,require_core_fields: bool) {
    #[derive(Default)]
    struct CoreFields {
      screen_res: bool,
    }
    let mut core=CoreFields::default();
    
    for (key,val) in headers {
      
      match key {
        b"screen_res"=>{
          self.server_screen_res=network::decode_from(val).expect("screen_res header too short");
          println!("server screen resolution is {}",self.server_screen_res);
          core.screen_res=true;
        },
        _=>{
          println!(
            "unknown server info header '{}' = '{}'",
            String::from_utf8_lossy(key),
            String::from_utf8_lossy(val),
          );
        },
      }
    }
    if require_core_fields {
      assert!(
        core.screen_res,
        "server-info is missing some core fields"
      );
    }
  }
  
  pub fn update<'a>(&mut self,headers: HeaderDecoder<'a>) {
    self.extend_from(headers,false);
  }
  pub fn from_decoder<'a>(headers: HeaderDecoder<'a>)->ServerInfo {
    let mut new: ServerInfo=unsafe{mem::uninitialized()};
    new.extend_from(headers,true);
    new
  }
  
  pub fn build(&self,config: &Config)->Setup {
    Setup::new(&self,config)
  }
}

pub struct HeaderDecoder<'a>(pub &'a [u8]);
impl<'a> Iterator for HeaderDecoder<'a> {
  type Item = (&'a [u8],&'a [u8]);
  fn next(&mut self)->Option<Self::Item> {
    if self.0.len()==0 {
      None
    }else{
      let mut consume=|len| {
        let slice=self.0.get(..len).expect("malformed header fields");
        self.0=&self.0[len..];
        slice
      };
      let key_len: u32=network::decode_from(consume(4)).unwrap();
      let key=consume(key_len as usize);
      let val_len: u32=network::decode_from(consume(4)).unwrap();
      let val=consume(val_len as usize);
      Some((key,val))
    }
  }
}

pub fn encode_header(buf: &mut Vec<u8>,key: &[u8],val: &[u8]) {
  network::encode_into(&mut *buf,&(key.len() as u32)).unwrap();
  buf.extend_from_slice(key);
  network::encode_into(&mut *buf,&(val.len() as u32)).unwrap();
  buf.extend_from_slice(val);
}
