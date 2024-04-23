use std::rc::Rc;

use mr::DataDescriptor;

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

pub type RawMappedAddress = libfabric_sys::fi_addr_t; 

#[repr(C)]
pub struct MappedAddress (libfabric_sys::fi_addr_t);

impl MappedAddress {
    pub fn unspec() -> Self {
        Self(FI_ADDR_UNSPEC)
    }
    pub(crate) fn from_raw_addr(addr: RawMappedAddress) -> Self {
        Self(addr)
    }

    pub(crate) fn raw_addr(&self) -> RawMappedAddress {
        self.0
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





pub fn rx_addr(addr: &MappedAddress, rx_index: i32, rx_ctx_bits: i32) -> Option<MappedAddress> {
    let ret = unsafe { libfabric_sys::inlined_fi_rx_addr(addr.raw_addr(), rx_index, rx_ctx_bits) };
    if ret == FI_ADDR_NOTAVAIL {
        None
    }
    else {
        Some(MappedAddress::from_raw_addr(ret))
    }
}

pub fn rx_addr_no_av(rx_index: i32, rx_ctx_bits: i32) -> Option<MappedAddress> {
    let ret = unsafe { libfabric_sys::inlined_fi_rx_addr(FI_ADDR_NOTAVAIL, rx_index, rx_ctx_bits) };
    if ret == FI_ADDR_NOTAVAIL {
        None
    }
    else {
        Some(MappedAddress::from_raw_addr(ret))
    }
}




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





pub struct Context {
    #[allow(dead_code)]

    c_val: libfabric_sys::fi_context,
}

impl Context {
    pub fn new() -> Self {
        Self {
            c_val : {
                libfabric_sys::fi_context { internal: [std::ptr::null_mut(); 4] }
            }
        }
    }
    
    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_context {
        &mut self.c_val
    }
}

impl Default for Context {
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
}

impl Default for Context2 {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BindImpl{}
pub trait Bind {
    fn inner(&self) -> Rc<dyn BindImpl>;
}



pub trait FdRetrievable{}
pub trait Waitable{}
pub trait Writable{}
pub trait WaitRetrievable{}


pub const MSG : usize = 0;
pub const RMA : usize = 1;
pub const TAG : usize = 2;
pub const ATOMIC : usize = 3;
pub const MCAST : usize = 4;
pub const NAMEDRXCTX : usize = 5;
pub const DRECV: usize = 6; 
pub const VMSG: usize = 7; 
pub const HMEM: usize = 8; 
pub const COLL: usize = 9; 
pub const XPU: usize = 10; 
pub const SEND: usize = 11; 
pub const RECV: usize = 12; 
pub const WRITE: usize = 13; 
pub const READ: usize = 14; 
pub const RWRITE: usize = 15; 
pub const RREAD: usize = 16;

pub const fn get<const N: usize>(index: usize, asked: &[usize]) -> bool {
    let mut i = 0;
    while i < N {
        if index == asked[i] {
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
macro_rules! caps_type_N {
    ($N: stmt, $($opt: ident),*) => {
        libfabric::infocapsoptions::InfoCaps<
        // set($N, MSG, $($opt),*), 
        {libfabric::get::<{$N}>(libfabric::MSG, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::RMA, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::TAG, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::ATOMIC, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::MCAST, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::NAMEDRXCTX, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::DRECV, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::VMSG, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::HMEM, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::COLL, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::XPU, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::SEND, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::RECV, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::WRITE, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::READ, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::RWRITE, &[$($opt),*])}, 
        {libfabric::get::<{$N}>(libfabric::RREAD, &[$($opt),*])}>
        
    };
}

#[macro_export]// MSG, RMA, TAG, ATOMIC, MCAST, NAMEDRXCTX, DRECV, VMSG, HMEM, COLL, XPU, SEND, RECV, WRITE, READ, RWRITE, RREAD
macro_rules!  caps_type{
    ($($opt: ident),*) => {
        libfabric::caps_type_N!(libfabric::count!($($opt)*), $($opt),*)
    };
}