#[cfg(all(feature="use-tokio", feature="use-async-std"))]
compile_error!("Features \"use-tokio\", \"use-async-std\" are mutually exclusive");

#[cfg(feature="thread-safe")]
use std::sync::OnceLock;
#[cfg(not(feature="thread-safe"))]
use std::cell::OnceCell;
#[cfg(feature="thread-safe")]
use std::sync::Arc;
#[cfg(not(feature="thread-safe"))]
use std::rc::Rc;

use enums::AddressVectorType;
#[cfg(feature="thread-safe")]
use parking_lot::RwLock;
 
#[cfg(not(feature="thread-safe"))]
use std::cell::RefCell;

#[cfg(feature="thread-safe")]
pub type MyRefCell<T> = RwLock<T>;
#[cfg(not(feature="thread-safe"))]
pub type MyRefCell<T> = RefCell<T>;

#[cfg(feature="thread-safe")]
pub type MyRc<T> = Arc<T>;
#[cfg(feature="thread-safe")]
pub type MyOnceCell<T> = OnceLock<T>;

#[cfg(not(feature="thread-safe"))]
pub type MyRc<T> = Rc<T>;
#[cfg(not(feature="thread-safe"))]
pub type MyOnceCell<T> = OnceCell<T>;


use av::{AddressVectorImplT, AddressVectorSetImpl};
use fid::AsRawFid;

pub mod ep;
pub mod domain;
pub mod eq;
pub mod fabric;
pub mod enums;
pub mod av;
pub mod mr;
pub mod sync;
pub mod cntr;
pub mod cq;
pub mod comm;
pub mod error;
pub mod xcontext;
pub mod eqoptions;
pub mod cqoptions;
pub mod cntroptions;
pub mod infocapsoptions;
pub mod nic;
pub mod info;
mod utils;
mod fid;
pub mod iovec;
pub mod msg;
#[cfg(any(feature="use-async-std", feature = "use-tokio"))]
pub mod async_;

#[derive(Clone)]
pub struct TableMappedAddress {
    raw_mapped_addr: libfabric_sys::fi_addr_t,
    av: AddressSource,
}

#[derive(Clone)]
pub struct UnspecMappedAddress {
    raw_mapped_addr: libfabric_sys::fi_addr_t,
}

#[derive(Clone)]
pub struct MapMappedAddress {
    raw_mapped_addr: u64,
    av: AddressSource,
}

#[derive(Clone, Debug)]
pub enum RawMappedAddress {
    Map(libfabric_sys::fi_addr_t),
    Table(libfabric_sys::fi_addr_t),
    Unspec(libfabric_sys::fi_addr_t)
}

impl RawMappedAddress {
    pub(crate) fn get(&self) -> libfabric_sys::fi_addr_t {
        match self {
            RawMappedAddress::Map(addr) => *addr,
            RawMappedAddress::Table(addr) => *addr,
            RawMappedAddress::Unspec(addr) => *addr,
        }
    }

    pub(crate) fn from_raw(av_type: AddressVectorType, raw_addr: libfabric_sys::fi_addr_t) -> RawMappedAddress {
        match av_type {
            AddressVectorType::Map => RawMappedAddress::Map(raw_addr),
            AddressVectorType::Table => RawMappedAddress::Table(raw_addr),
            AddressVectorType::Unspec => panic!("Unspecified address type"),
        }
    }
}

#[derive(Clone)]
pub(crate) enum AddressSource {
    Av(MyRc<dyn AddressVectorImplT>),
    AvSet(MyRc<AddressVectorSetImpl>)
}

/// Owned wrapper around a libfabric `fi_addr_t`.
/// 
/// This type wraps an instance of a `fi_addr_t`, in order to prevent modification and to monitor its lifetime.
/// It is usually generated by an [crate::av::AddressVector] after inserting an [crate::ep::Address].
/// For more information see the libfabric [documentation](https://ofiwg.github.io/libfabric/v1.19.0/man/fi_av.3.html).
/// 
/// Note that other objects that it will extend the respective [`crate::av::AddressVector`]'s (if any) lifetime until they
/// it is dropped.
#[derive(Clone)]
pub enum MappedAddress {
    Unspec(UnspecMappedAddress),
    Map(MapMappedAddress),
    Table(TableMappedAddress),
}

impl MappedAddress {

    // pub(crate) fn from_raw_addr_trait(addr: RawMappedAddress, av: &MyRc<dyn AddressVectorImplT>) -> Self {
    //     let avcell = OnceCell::new();
        
    //     if avcell.set(AddressSource::Av(av.clone())).is_err() {
    //         panic!("MappedAddress is already set");
    //     } 
        
    //     Self {
    //         addr,
    //         av: avcell,
    //     }
    // }
    
    pub(crate) fn from_raw_addr(addr: RawMappedAddress, av: AddressSource) -> Self {
        match addr {
            RawMappedAddress::Map(addr) => Self::Map(MapMappedAddress{raw_mapped_addr: addr, av: av}),
            RawMappedAddress::Table(addr) => Self::Table(TableMappedAddress{raw_mapped_addr: addr, av: av}),
            RawMappedAddress::Unspec(addr) => Self::Unspec(UnspecMappedAddress{raw_mapped_addr: addr}),
        }
    }

    pub(crate) fn from_raw_addr_no_av(addr: RawMappedAddress) -> Self {
        match addr {
            RawMappedAddress::Unspec(addr) => Self::Unspec(UnspecMappedAddress{raw_mapped_addr: addr}),
            _ => panic!("Addresses mapped by an AV")
        }
    }

    pub(crate) fn raw_addr(&self) -> libfabric_sys::fi_addr_t {
        match self {
            Self::Map(t) => t.raw_mapped_addr,
            Self::Table(m) => m.raw_mapped_addr,
            Self::Unspec(u) => u.raw_mapped_addr,
        }
    }

    pub fn rx_addr(&self, rx_index: i32, rx_ctx_bits: i32) -> Result<MappedAddress, crate::error::Error> {
        let ret = unsafe { libfabric_sys::inlined_fi_rx_addr(self.raw_addr(), rx_index, rx_ctx_bits) };
        if ret == FI_ADDR_NOTAVAIL || ret == FI_ADDR_UNSPEC {
            return Err(crate::error::Error::from_err_code(libfabric_sys::FI_EADDRNOTAVAIL));
        }
    
        Ok(match self {
            Self::Map(m) => 
                Self::Map(
                    MapMappedAddress{raw_mapped_addr: ret, av: m.av.clone()}
                )
            ,
            Self::Table(t) => 
                Self::Table(
                    TableMappedAddress{raw_mapped_addr: ret, av: t.av.clone()}
                )
            ,
            Self::Unspec(_) => 
                Self::Unspec(
                    UnspecMappedAddress{raw_mapped_addr: ret}
                )
            ,
        })
    }
    
}

pub type DataType = libfabric_sys::fi_datatype;
pub type CollectiveOp = libfabric_sys::fi_collective_op;
const FI_ADDR_NOTAVAIL : u64 = u64::MAX;
const FI_KEY_NOTAVAIL : u64 = u64::MAX;
const FI_ADDR_UNSPEC : u64 = u64::MAX;


// pub struct Stx {

//     #[allow(dead_code)]
//     c_stx: *mut libfabric_sys::fid_stx,
// }

// impl Stx {
//     pub(crate) fn new<T0>(domain: &crate::domain::Domain, mut attr: crate::TxAttr, context: &mut T0) -> Result<Stx, error::Error> {
//         let mut c_stx: *mut libfabric_sys::fid_stx = std::ptr::null_mut();
//         let c_stx_ptr: *mut *mut libfabric_sys::fid_stx = &mut c_stx;
//         let err = unsafe { libfabric_sys::inlined_fi_stx_context(domain.c_domain, attr.get_mut(), c_stx_ptr, context as *mut T0 as *mut std::ffi::c_void) };

//         if err != 0 {
//             Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
//         }
//         else {
//             Ok(
//                 Self { c_stx }
//             )
//         }

//     }
// }

// pub struct SrxAttr {
//     c_attr: libfabric_sys::fi_srx_attr,
// }

// impl SrxAttr {
//     pub(crate) fn get(&self) -> *const libfabric_sys::fi_srx_attr {
//         &self.c_attr
//     }

//     pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_srx_attr {
//         &mut self.c_attr
//     }
// }










// struct fi_param {
// 	const char *name;
// 	enum fi_param_type type;
// 	const char *help_string;
// 	const char *value;
// };

// int fi_getparams(struct fi_param **params, int *count);
// void fi_freeparams(struct fi_param *params);


// pub struct Param {
//     c_param : libfabric_sys::fi_param,
// }

// pub fn get_params() -> Vec<Param> {
//     let mut len = 0 as i32;
//     let len_ptr : *mut i32 = &mut len;
//     let mut c_params: *mut libfabric_sys::fi_param = std::ptr::null_mut();
//     let mut c_params_ptr: *mut *mut libfabric_sys::fi_param = &mut c_params;
    
//     let err = libfabric_sys::fi_getparams(c_params_ptr, len_ptr);
//     if err != 0 {
//         panic!("fi_getparams failed {}", err);
//     }

//     let mut params = Vec::<Param>::new();
//     for i  in 0..len {
//         params.push(Param { c_param: unsafe{ c_params.add(i as usize) } });
//     }

//     params
// }


// pub struct Param {
//     c_param: libfabric_sys::fi_param,
// }





pub struct Context1 {
    #[allow(dead_code)]
    c_val: libfabric_sys::fi_context,
}

impl Context1 {
    pub fn new() -> Self {
        Self {
            c_val : {
                libfabric_sys::fi_context { internal: [std::ptr::null_mut(); 4] }
            }
        }
    }
    
    #[allow(dead_code)]
    pub(crate) fn get(&self) -> *const libfabric_sys::fi_context {
        &self.c_val
    }
    
    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_context {
        &mut self.c_val
    }
}

impl Default for Context1 {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Context2 {
    c_val: libfabric_sys::fi_context2,
}

impl Context2 {
    pub fn new() -> Self {
        Self {
            c_val : {
                libfabric_sys::fi_context2 { internal: [std::ptr::null_mut(); 8] }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_context2 {
        &mut self.c_val
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self) -> *const libfabric_sys::fi_context2 {
        &self.c_val
    }
}

impl Default for Context2 {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Context {
    Context1(Context1),
    Context2(Context2),
}

impl Context {
    fn inner_mut(&mut self) -> *mut std::ffi::c_void {
        match self {
            Context::Context1(ctx) => ctx.get_mut() as *mut std::ffi::c_void,
            Context::Context2(ctx) => ctx.get_mut() as *mut std::ffi::c_void,
        }
    }

    fn inner(&self) -> *const std::ffi::c_void {
        match self {
            Context::Context1(ctx) => ctx.get() as *const std::ffi::c_void,
            Context::Context2(ctx) => ctx.get() as *const std::ffi::c_void,
        }
    }
}


// pub trait BindImpl: AsRawFid {}
pub trait Bind {
    fn inner(&self) -> MyRc<dyn AsRawFid>;
}



pub trait FdRetrievable{}
pub trait Waitable{}
pub trait Writable{}
pub trait WaitRetrievable{}


pub enum FabInfoCaps {
    MSG =           0,
    RMA =           1,
    TAG =           2,
    ATOMIC =        3,
    MCAST =         4,
    NAMEDRXCTX =    5,
    DRECV =         6, 
    VMSG =          7, 
    HMEM =          8, 
    COLL =          9, 
    XPU =           10, 
    SEND =          11, 
    RECV =          12, 
    WRITE =         13, 
    READ =          14, 
    RWRITE =        15, 
    RREAD =         16,
}


impl FabInfoCaps {
    pub const fn value(&self) -> usize{
        match self {
            FabInfoCaps::MSG => FabInfoCaps::MSG as usize,
            FabInfoCaps::RMA => FabInfoCaps::RMA as usize,
            FabInfoCaps::TAG => FabInfoCaps::TAG as usize,
            FabInfoCaps::ATOMIC => FabInfoCaps::ATOMIC as usize,
            FabInfoCaps::MCAST => FabInfoCaps::MCAST as usize,
            FabInfoCaps::NAMEDRXCTX => FabInfoCaps::NAMEDRXCTX as usize,
            FabInfoCaps::DRECV => FabInfoCaps::DRECV as usize,
            FabInfoCaps::VMSG => FabInfoCaps::VMSG as usize,
            FabInfoCaps::HMEM => FabInfoCaps::HMEM as usize,
            FabInfoCaps::COLL => FabInfoCaps::COLL as usize,
            FabInfoCaps::XPU => FabInfoCaps::XPU as usize,
            FabInfoCaps::SEND => FabInfoCaps::SEND as usize,
            FabInfoCaps::RECV => FabInfoCaps::RECV as usize,
            FabInfoCaps::WRITE => FabInfoCaps::WRITE as usize,
            FabInfoCaps::READ => FabInfoCaps::READ as usize,
            FabInfoCaps::RWRITE => FabInfoCaps::RWRITE as usize,
            FabInfoCaps::RREAD => FabInfoCaps::RREAD as usize,
        }
    }
}
pub enum SyncCaps {
    WAIT =      0,
    RETRIEVE =  1,
    FD =        2,
}

pub use SyncCaps as CntrCaps;
pub use SyncCaps as CqCaps;

pub enum EqCaps {
    WAIT =      0,
    RETRIEVE =  1,
    FD =        2,
    WRITE =     3,
}

impl SyncCaps {
    pub const fn value(&self) -> usize{
        match self {
            SyncCaps::WAIT => SyncCaps::WAIT as usize,
            SyncCaps::RETRIEVE => SyncCaps::RETRIEVE as usize,
            SyncCaps::FD => SyncCaps::FD as usize,
        }
    }
}

impl EqCaps {
    pub const fn value(&self) -> usize{
        match self {
            EqCaps::WAIT => EqCaps::WAIT as usize,
            EqCaps::RETRIEVE => EqCaps::RETRIEVE as usize,
            EqCaps::FD => EqCaps::FD as usize,
            EqCaps::WRITE => EqCaps::WRITE as usize,
        }
    }
}

pub const fn get_eq<const N: usize>(index: EqCaps, asked: &[EqCaps]) -> bool {
    let mut i = 0;
    while i < N {
        if index.value() == asked[i].value() {
            return true
        }
        i+=1;
    }
    false
}

pub const fn get_sync<const N: usize>(index: SyncCaps, asked: &[SyncCaps]) -> bool {
    let mut i = 0;
    while i < N {
        if index.value() == asked[i].value() {
            return true
        }
        i+=1;
    }
    false
}

pub const fn get_info<const N: usize>(index: FabInfoCaps, asked: &[FabInfoCaps]) -> bool {
    let mut i = 0;
    while i < N {
        if index.value() == asked[i].value() {
            return true
        }
        i+=1;
    }
    false
}

#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules! count {
    () => (0usize);
    ( $x:tt $($xs:tt)* ) => (1usize + libfabric::count!($($xs)*));
}
// macro_rules! set {
//     ($N: stmt, $opt: ident $opts: expr, ) => {
//         {get::<{$N}>($opt, &[$($opts),*])}
//     };
// }
#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules! info_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::infocapsoptions::InfoCaps<
        // set($N, MSG, $($opt),*), 
        {libfabric::get_info::<{$N}>(FabInfoCaps::MSG, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::RMA, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::TAG, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::ATOMIC, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::MCAST, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::NAMEDRXCTX, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::DRECV, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::VMSG, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::HMEM, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::COLL, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::XPU, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::SEND, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::RECV, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::WRITE, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::READ, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::RWRITE, &[$($opt),*])}, 
        {libfabric::get_info::<{$N}>(FabInfoCaps::RREAD, &[$($opt),*])}>
        
    };
}

#[macro_export]// CQ: WAIT, RETRIEVE, FD
macro_rules! cq_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::cq::CompletionQueueImpl<
        // set($N, MSG, $($opt),*), 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::WAIT, &[$($opt),*])}, 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::RETRIEVE, &[$($opt),*])}, 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::FD, &[$($opt),*])}, >
    };
}

#[macro_export]// EQ: WRITE, WAIT, RETRIEVE, FD
macro_rules! eq_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::eq::EventQueueImpl<
        // set($N, MSG, $($opt),*), 
        {libfabric::get_eq::<{$N}>(libfabric::EqCaps::WRITE, &[$($opt),*])}, 
        {libfabric::get_eq::<{$N}>(libfabric::EqCaps::WAIT, &[$($opt),*])}, 
        {libfabric::get_eq::<{$N}>(libfabric::EqCaps::RETRIEVE, &[$($opt),*])}, 
        {libfabric::get_eq::<{$N}>(libfabric::EqCaps::FD, &[$($opt),*])},> 
    };
}

#[macro_export]// CNTR: WAIT, RETRIEVE, FD
macro_rules! cntr_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::cntr::CounterImpl<
        // set($N, MSG, $($opt),*), 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::WAIT, &[$($opt),*])}, 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::RETRIEVE, &[$($opt),*])}, 
        {libfabric::get_sync::<{$N}>(libfabric::SyncCaps::FD, &[$($opt),*])}, >
    };
}





#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  info_caps_type{
    ($($opt: expr),*) => {
        libfabric::info_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  cq_caps_type{
    ($($opt: expr),*) => {
        libfabric::cq_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  eq_caps_type{
    ($($opt: expr),*) => {
        libfabric::eq_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  cntr_caps_type{
    ($($opt: expr),*) => {
        libfabric::cntr_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

#[cfg(any(feature="use-async-std", feature = "use-tokio"))]
#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules! async_cq_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::async_::cq::AsyncCompletionQueueImpl
    };
}

#[cfg(any(feature="use-async-std", feature = "use-tokio"))]
#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules! async_eq_caps_type_N {
    ($N: stmt, $($opt: expr),*) => {
        libfabric::async_::eq::AsyncEventQueueImpl<
        // set($N, MSG, $($opt),*), 
        {libfabric::get_eq::<{$N}>(libfabric::EqCaps::WRITE, &[$($opt),*])},> 
    };
}

#[cfg(any(feature="use-async-std", feature = "use-tokio"))]
#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  async_cq_caps_type{
    ($($opt: expr),*) => {
        libfabric::async_cq_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

#[cfg(any(feature="use-async-std", feature = "use-tokio"))]
#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  async_eq_caps_type{
    ($($opt: expr),*) => {
        libfabric::async_eq_caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}

pub trait AsFiType {
    fn as_fi_datatype() -> libfabric_sys::fi_datatype;
}

macro_rules! impl_as_fi_type {
    ($(($rtype: ty, $fitype: path)),*) => {
        $(impl AsFiType for $rtype {
            fn as_fi_datatype() -> libfabric_sys::fi_datatype {
                $fitype
            }
        })*
    };
}

impl_as_fi_type!(
    ((), libfabric_sys::fi_datatype_FI_VOID),
    (i8, libfabric_sys::fi_datatype_FI_INT8),
    (i16, libfabric_sys::fi_datatype_FI_INT16),
    (i32, libfabric_sys::fi_datatype_FI_INT32),
    (i64, libfabric_sys::fi_datatype_FI_INT64),
    (i128, libfabric_sys::fi_datatype_FI_INT128),
    (u8, libfabric_sys::fi_datatype_FI_UINT8),
    (u16, libfabric_sys::fi_datatype_FI_UINT16),
    (u32, libfabric_sys::fi_datatype_FI_UINT32),
    (u64, libfabric_sys::fi_datatype_FI_UINT64),
    (u128, libfabric_sys::fi_datatype_FI_UINT128),
    (f32, libfabric_sys::fi_datatype_FI_FLOAT),
    (f64, libfabric_sys::fi_datatype_FI_DOUBLE)
);

impl AsFiType for usize {
    fn as_fi_datatype() -> libfabric_sys::fi_datatype {
        if std::mem::size_of::<usize>() == 8 {libfabric_sys::fi_datatype_FI_UINT64}
        else if std::mem::size_of::<usize>() == 4 {libfabric_sys::fi_datatype_FI_UINT32}
        else if std::mem::size_of::<usize>() == 2 {libfabric_sys::fi_datatype_FI_UINT16}
        else if std::mem::size_of::<usize>() == 1 {libfabric_sys::fi_datatype_FI_UINT8}
        else {panic!("Unhandled usize datatype size")}
    }
}

impl AsFiType for isize {
    fn as_fi_datatype() -> libfabric_sys::fi_datatype {
        if std::mem::size_of::<isize>() == 8 {libfabric_sys::fi_datatype_FI_INT64}
        else if std::mem::size_of::<isize>() == 4 {libfabric_sys::fi_datatype_FI_INT32}
        else if std::mem::size_of::<isize>() == 2 {libfabric_sys::fi_datatype_FI_INT16}
        else if std::mem::size_of::<isize>() == 1 {libfabric_sys::fi_datatype_FI_INT8}
        else {panic!("Unhandled isize datatype size")}
    }
}