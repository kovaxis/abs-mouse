use std::{
  ops::{Add,AddAssign,Sub,SubAssign,Mul,Div,Index,IndexMut},
  cmp::{Ordering},
  mem,
  fmt::{self,Display},
};

macro_rules! pair {
  ($idx:pat=> $($tt:tt)*)=>{
    $crate::rect::Pair([{
      let $idx=$crate::rect::Axis::X;
      $($tt)*
    },{
      let $idx=$crate::rect::Axis::Y;
      $($tt)*
    }])
  };
}

pub trait Abs {
  fn abs(self)->Self;
  fn signum(self)->Self;
}
macro_rules! impl_abs {
  ($($ty:tt)*)=>{$(
    impl Abs for $ty {
      fn abs(self)->Self {self.abs()}
      fn signum(self)->Self {self.signum()}
    }
  )*};
}
impl_abs!(i8 i16 i32 i64 f32 f64);

#[derive(Serialize,Deserialize,Copy,Clone,Debug)]
pub struct Rect<T> {
  pub min: Pair<T>,
  pub max: Pair<T>,
}
impl<T> Rect<T> where
  T: Copy+Add<Output=T>+Sub<Output=T>+Mul<Output=T>+Div<Output=T>+AddAssign+SubAssign+PartialOrd+Abs+From<u8>,
{
  pub fn devirtualize(&self,idx: Axis)->Axis {
    if self.should_swap() {
      idx.swap()
    }else{
      idx
    }
  }
  
  ///Compute the minimum value in the __virtual__ given axis.
  pub fn virtual_min(&self,idx: Axis)->T {
    self.min[self.devirtualize(idx)]
  }
  ///Compute the maximum value in the __virtual__ given axis.
  pub fn virtual_max(&self,idx: Axis)->T {
    self.max[self.devirtualize(idx)]
  }
  ///Compute the size of the rectangle in a given axis.
  pub fn virtual_size(&self,idx: Axis)->T {
    self.virtual_max(idx)-self.virtual_min(idx)
  }
  ///Compute the size of the rectangle in the __virtual__ X axis.
  pub fn virtual_width(&self)->T {self.virtual_size(Axis::X)}
  ///Compute the size of the rectangle in the __virtual__ Y axis.
  pub fn virtual_height(&self)->T {self.virtual_size(Axis::Y)}
  
  ///Compute the 'aspect ratio' of the rectangle, `width/height`.
  pub fn aspect_ratio(&self)->T {(self.virtual_width()/self.virtual_height()).abs()}
  ///Compute the 'inverse aspcect ratio' of the rectangle, `height/width`.
  pub fn inv_aspect_ratio(&self)->T {(self.virtual_height()/self.virtual_width()).abs()}
  ///Get how does the absolute width compare to the absolute height.
  pub fn aspect(&self)->Aspect {
    match self.virtual_width().abs().partial_cmp(&self.virtual_height().abs()) {
      None=>panic!("failed to compare width and height"),
      Some(Ordering::Greater)=>Aspect::Landscape,
      Some(Ordering::Equal)=>Aspect::Square,
      Some(Ordering::Less)=>Aspect::Portrait,
    }
  }
  
  ///Is the max larger than the min in the given axis?
  pub fn sign(&self,idx: Axis)->Sign {
    Sign::from_cmp(self.min[idx],self.max[idx])
  }
  ///Get a pair of signs representing the rectangle configuration.
  pub fn sign_pair(&self)->Pair<Sign> {
    pair!(i=> self.sign(i))
  }
  ///Whether input weights should swap their axes.
  pub fn should_swap(&self)->bool {
    self.sign(Axis::X)!=self.sign(Axis::Y)
  }
  
  ///Rotate the rectangle orientation by +90° (counterclockwise) in a standard cartesian system.
  ///In a computer-style reversed-Y system rotates by 90° clockwise.
  pub fn rotate_positive(&mut self) {
    self.min.swap();
    self.max.swap();
    mem::swap(&mut self.min[Axis::X],&mut self.max[Axis::Y]);
  }
  ///Rotate the rectangle orientation by -90° (clockwise) in a standard cartesian system.
  ///In a computer-style reversed-Y system rotates by 90° counterclockwise.
  pub fn rotate_negative(&mut self) {
    self.min.swap();
    self.max.swap();
    mem::swap(&mut self.min[Axis::Y],&mut self.max[Axis::Y]);
  }
  ///Does a full 180° rotation on the rectangle.
  pub fn rotate_full(&mut self) {
    mem::swap(&mut self.min,&mut self.max);
  }
  
  ///Resize a virtual axis to the given target size, keeping the rectangle centered.
  pub fn resize_virtual_axis(&mut self,axis: Axis,target: T) {
    let raw_idx=self.devirtualize(axis);
    let diff=(target.abs()-self.virtual_size(axis).abs())*self.virtual_size(axis).signum();
    self.min[raw_idx]-=diff/T::from(2);
    self.max[raw_idx]+=diff/T::from(2);
  }
}
impl<T: Copy> Rect<T> {
  pub fn map<F: FnMut(T)->U,U>(self,mut f: F)->Rect<U> {
    Rect{min: pair!(i=> f(self.min[i])),max: pair!(i=> f(self.max[i]))}
  }
  pub fn cast<U>(self)->Rect<U> where T: Into<U> {
    self.map(|t| t.into())
  }
}
impl Rect<f32> {
  ///Map a system coordinate point into normalized aligned coordinates.
  pub fn normalizer(&self)->Mapping {
    let inv_size=pair!(i=> 1.0/(self.max[i]-self.min[i]));
    let offset=pair!(i=> -self.min[i]*inv_size[i]);
    let swap=self.should_swap();
    Mapping{swap,multiplier: inv_size,offset}
  }
  ///Map a normalized system-aligned point into a real point.
  pub fn denormalizer(&self)->Mapping {
    let mut multiplier=pair!(i=> self.max[i]-self.min[i]);
    let mut offset=self.min;
    let swap=self.should_swap();
    if swap {
      multiplier.swap();
      offset.swap();
    }
    Mapping{multiplier,offset,swap}
  }
}
impl<T: Display> Display for Rect<T> {
  fn fmt(&self,f: &mut fmt::Formatter)->fmt::Result {
    write!(f,"{} -> {}",self.min,self.max)
  }
}

#[derive(Serialize,Deserialize,Copy,Clone,Debug,Default,Hash)]
pub struct Pair<T>(pub [T; 2]);
impl<T> Pair<T> {
  ///Swaps the x and y axes.
  pub fn swap(&mut self) {self.0.swap(0,1)}
}
impl<T> Index<Axis> for Pair<T> {
  type Output=T;
  fn index(&self,idx: Axis)->&T {&self.0[idx.as_index()]}
}
impl<T> IndexMut<Axis> for Pair<T> {
  fn index_mut(&mut self,idx: Axis)->&mut T {&mut self.0[idx.as_index()]}
}
impl<T: Display> Display for Pair<T> {
  fn fmt(&self,f: &mut fmt::Formatter)->fmt::Result {
    write!(f,"[{},{}]",self[Axis::X],self[Axis::Y])
  }
}

#[derive(Copy,Clone,Debug,Hash,PartialEq,Eq)]
#[repr(usize)]
pub enum Axis {
  X,
  Y,
}
impl Axis {
  fn as_index(self)->usize {self as usize}
  ///Swaps x to y and y to x.
  pub fn swap(self)->Axis {match self {
    Axis::X=>Axis::Y,
    Axis::Y=>Axis::X,
  }}
}

///Classifies aspect ratios in >1 (`Landscape`), =1 (`Square`) and <1 (`Portrait`)
#[derive(Copy,Clone,Debug,Hash)]
pub enum Aspect {
  Landscape,
  Square,
  Portrait,
}
impl PartialEq for Aspect {
  fn eq(&self,rhs: &Self)->bool {
    match (*self,*rhs) {
      (Aspect::Landscape,Aspect::Portrait)=>false,
      (Aspect::Portrait,Aspect::Landscape)=>false,
      _ => true,
    }
  }
}
impl Eq for Aspect {}

#[derive(Copy,Clone,PartialEq,Eq,Debug,Hash)]
pub enum Sign {
  Positive,
  Negative,
}
impl Sign {
  pub fn from_cmp<T: PartialOrd>(min: T,max: T)->Sign {
    if max>=min {Sign::Positive}else{Sign::Negative}
  }
}

///Map a point to another.
pub struct Mapping {
  ///Apply a multiplier to each axis of the pair.
  pub multiplier: Pair<f32>,
  ///Apply an offset to each axis of the pair, after the multiplier is applied.
  pub offset: Pair<f32>,
  ///Swap the output point after everything.
  pub swap: bool,
}
impl Mapping {
  ///Chain a mapping after the other to create a single new equivalent mapping.
  pub fn chain(&self,next: &Mapping)->Mapping {
    //Swaps cancel each other
    let swap={self.swap!=next.swap};
    
    let mul0=self.multiplier;
    let off0=self.offset;
    
    //If the first mapping should output a swapped pair, swap the second multiplier to emulate
    //the swap (without actually swapping in between, which we can't do).
    let mut mul1=next.multiplier;
    let mut off1=next.offset;
    if self.swap {
      mul1.swap();
      off1.swap();
    }
    
    //Calculate final multiplier and offset according to:
    // (x*mul0+off0)*mul1+off1
    // x*mul0*mul1+off0*mul1+off1
    let multiplier=pair!(i=> mul0[i]*mul1[i]);
    let offset=pair!(i=> off0[i]*mul1[i]+off1[i]);
    
    Mapping{multiplier,offset,swap}
  }
  
  ///Apply the mapping on a point.
  pub fn apply(&self,mut input: Pair<f32>)->Pair<f32> {
    if self.swap {input.swap()}
    pair!(i=> input[i]*self.multiplier[i]+self.offset[i])
  }
}
