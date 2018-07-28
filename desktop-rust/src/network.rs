use prelude::*;
use std::{
  net::{TcpStream,UdpSocket},
  cell::{RefCell},
};
use bincode;

pub fn decode_from<T,R: Read>(read: R)->Result<T> where for<'a> T: Deserialize<'a> {
  let mut cfg=bincode::config();
  cfg.big_endian();
  Ok(cfg.deserialize_from(read)?)
}
pub fn encode_into<T: Serialize,W: Write>(write: W,obj: &T)->Result<()> {
  let mut cfg=bincode::config();
  cfg.big_endian();
  Ok(cfg.serialize_into(write,obj)?)
}

pub fn f32_to_bytes(float: f32)->[u8; 4] {
  let mut bytes=[0; 4];
  encode_into(&mut bytes[..],&float);
  bytes
}

#[derive(Serialize,Deserialize,Clone,Debug)]
pub enum Remote {
  Tcp(String,u16),
  Udp(String,u16),
}
impl Remote {
  pub fn connect(&self)->Result<Box<Connection>> {
    match self {
      Remote::Tcp(host,port)=>{
        let stream=TcpStream::connect((&**host,*port))?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(None)?;
        stream.set_nonblocking(false)?;
        Ok(Box::new(stream))
      },
      Remote::Udp(host,port)=>{
        let sock=UdpSocket::bind(("localhost",0))?;
        sock.connect((&**host,*port))?;
        Ok(Box::new(sock))
      },
    }
  }
}
impl fmt::Display for Remote {
  fn fmt(&self,f: &mut fmt::Formatter)->fmt::Result {
    match self {
      Remote::Tcp(host,port)=>write!(f,"tcp/{}/{}",host,port),
      Remote::Udp(host,port)=>write!(f,"udp/{}/{}",host,port),
    }
  }
}

pub trait Connection {
  fn send(&mut self,&[u8])->Result<()>;
  fn recv(&mut self,&mut Vec<u8>)->Result<()>;
}

///Call a closure with mutable access to an empty cached network buffer.
///If `net_buffer` is called before the closure returns it will `panic`.
pub struct NetBuffer(RefCell<Vec<u8>>);
impl LocalBuffer for ::std::thread::LocalKey<NetBuffer> {
  type Inner = Vec<u8>;
  fn borrow<F: FnOnce(&mut Vec<u8>)->T,T>(&'static self,f: F)->T {
    self.with(|net_buffer| {
      let mut net_buffer=net_buffer.0.borrow_mut();
      net_buffer.clear();
      f(&mut *net_buffer)
    })
  }
}
impl Default for NetBuffer {
  fn default()->NetBuffer {NetBuffer(RefCell::new(Vec::with_capacity(65536)))}
}

pub trait LocalBuffer {
  type Inner;
  fn borrow<F: FnOnce(&mut Self::Inner)->T,T>(&'static self,f: F)->T;
}

thread_local!{
  static NET_BUFFER: NetBuffer=Default::default();
}

impl Connection for TcpStream {
  fn send(&mut self,data: &[u8])->Result<()> {
    NET_BUFFER.borrow(|buf| {
      let len: u32=data.len() as u32;
      encode_into(&mut *buf,&len)?;
      buf.extend_from_slice(data);
      self.write_all(&buf)?;
      Ok(())
    })
  }
  fn recv(&mut self,buf: &mut Vec<u8>)->Result<()> {
    //Get message length from stream
    let mut len=[0; 4];
    self.read_exact(&mut len)?;
    let len: u32=decode_from(&len[..])?;
    let len=len as usize;
    //Get a slice of uninitialized data from buffer
    buf.clear();
    buf.reserve(len);
    unsafe{buf.set_len(len)}
    let slice=&mut buf[..len];
    //Read into uninitialized slice
    self.read_exact(slice)?;
    Ok(())
  }
}

impl Connection for UdpSocket {
  fn send(&mut self,data: &[u8])->Result<()> {
    UdpSocket::send(self,data)?;
    Ok(())
  }
  fn recv(&mut self,buf: &mut Vec<u8>)->Result<()> {
    //Get an uninitialized slice as large as possible from the buffer
    let slice=unsafe{
      let len=buf.capacity();
      buf.set_len(len);
      &mut buf[..]
    };
    UdpSocket::recv(self,slice)?;
    Ok(())
  }
}
