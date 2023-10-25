use core::panic;
use std::ffi::c_void;

use crate::{FID, Address, DataType};

pub struct PassiveEndPoint {
    pub(crate) c_pep: *mut libfabric_sys::fid_pep,
}

impl PassiveEndPoint {
    pub fn new(fabric: &crate::fabric::Fabric, info: &crate::InfoEntry) -> Self {
        let mut c_pep: *mut libfabric_sys::fid_pep = std::ptr::null_mut();
        let c_pep_ptr: *mut *mut libfabric_sys::fid_pep = &mut c_pep;
        let err = unsafe { libfabric_sys::inlined_fi_passive_ep(fabric.c_fabric, info.c_info, c_pep_ptr, std::ptr::null_mut()) };
        
        if err != 0 {
            panic!("fi_passive_ep failed {}", err);
        }
        
        Self { c_pep }
    }

    pub fn new_with_context<T0>(fabric: &crate::fabric::Fabric, info: &crate::InfoEntry, context: &mut T0) -> Self {
        let mut c_pep: *mut libfabric_sys::fid_pep = std::ptr::null_mut();
        let c_pep_ptr: *mut *mut libfabric_sys::fid_pep = &mut c_pep;
        let err = unsafe { libfabric_sys::inlined_fi_passive_ep(fabric.c_fabric, info.c_info, c_pep_ptr, context as *mut T0 as *mut std::ffi::c_void) };
        
        if err != 0 {
            panic!("fi_passive_ep failed {}", err);
        }
        
        Self { c_pep }
    }
    
    pub fn bind(&self, fid: &impl FID, flags: u64) {
        let err = unsafe { libfabric_sys::inlined_fi_pep_bind(self.c_pep,fid.fid(), flags) };
        
        if err != 0 {
            panic!("fi_pep_bind failed {}", err);
        }
    }

    pub fn listen(&self) {
        let err = unsafe {libfabric_sys::inlined_fi_listen(self.c_pep)};
        
        if err != 0 {
            panic!("fi_listen failed {}", err);
        }
    }

    pub fn reject<T0>(&self, fid: &impl FID, params: &[T0]) {
        let err = unsafe {libfabric_sys::inlined_fi_reject(self.c_pep, fid.fid(), params.as_ptr() as *const std::ffi::c_void, params.len())};

        if err != 0 {
            panic!("fi_reject failed {}", err);
        }

    }
}

impl Endpoint {

    pub fn new<T0>(domain: &crate::domain::Domain, info: &crate::InfoEntry, context: &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_endpoint(domain.c_domain, info.c_info, c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void) };

        if err != 0 {
            panic!("fi_endpoint failed {}", err);
        }

        Self { c_ep }
    }

    pub fn new2<T0>(domain: &crate::domain::Domain, info: &crate::InfoEntry, flags: u64, context: &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_endpoint2(domain.c_domain, info.c_info, c_ep_ptr, flags, context as *mut T0 as *mut std::ffi::c_void) };

        if err != 0 {
            panic!("fi_endpoint2 failed {}", err);
        }

        Self { c_ep }
    }

    pub fn new_scalable(domain: &crate::domain::Domain, info: &crate::InfoEntry) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_scalable_ep(domain.c_domain, info.c_info, c_ep_ptr, std::ptr::null_mut()) };

        if err != 0 {
            panic!("fi_scalable_ep failed {}", err);
        }

        Self { c_ep }
    }

    pub fn new_scalable_with_context<T0>(domain: &crate::domain::Domain, info: &crate::InfoEntry, context : &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_scalable_ep(domain.c_domain, info.c_info, c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void) };

        if err != 0 {
            panic!("fi_scalable_ep failed {}", err);
        }

        Self { c_ep }
    }

    pub(crate) fn from_attr(domain: &crate::domain::Domain, mut rx_attr: crate::RxAttr) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_srx_context(domain.c_domain, rx_attr.get_mut(), c_ep_ptr,  std::ptr::null_mut()) };

        if err != 0 {
            panic!("fi_srx_context failed {}", err);
        }

        Self { c_ep }        
    }

    pub(crate) fn from_attr_with_context<T0>(domain: &crate::domain::Domain, mut rx_attr: crate::RxAttr, context: &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_srx_context(domain.c_domain, rx_attr.get_mut(), c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void) };

        if err != 0 {
            panic!("fi_srx_context failed {}", err);
        }

        Self { c_ep }        
    }

    pub fn bind(&self, fid: &impl FID, flags: u64) {
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.c_ep,fid.fid(), flags) };
        
        if err != 0 {
            panic!("fi_ep_bind failed {}", err);
        }
    } 

    pub fn enable(&self) {
        let err = unsafe { libfabric_sys::inlined_fi_enable(self.c_ep) };
        
        if err != 0 {
            panic!("fi_enable failed {}", err);
        }
    }

    pub fn alias(&self, flags: u64) -> Endpoint {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;
        let err = unsafe { libfabric_sys::inlined_fi_ep_alias(self.c_ep, c_ep_ptr, flags) };
        
        if err != 0 {
            panic!("fi_ep_alias failed {}", err);
        }
        Endpoint { c_ep }
    }

    pub fn tx_context(&self, idx: i32, mut txattr: crate::TxAttr) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;

        let err = unsafe {libfabric_sys::inlined_fi_tx_context(self.c_ep, idx, txattr.get_mut(), c_ep_ptr, std::ptr::null_mut())};
        
        if err != 0 {
            panic!("fi_tx_context failed {}", err);
        }

        Self { c_ep }
    }

    pub fn tx_context_with_context<T0>(&self, idx: i32, mut txattr: crate::TxAttr, context : &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;

        let err = unsafe {libfabric_sys::inlined_fi_tx_context(self.c_ep, idx, txattr.get_mut(), c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void)};
        
        if err != 0 {
            panic!("fi_tx_context failed {}", err);
        }

        Self { c_ep }
    }

    pub fn rx_context(&self, idx: i32, mut rxattr: crate::RxAttr) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;

        let err = unsafe {libfabric_sys::inlined_fi_rx_context(self.c_ep, idx, rxattr.get_mut(), c_ep_ptr, std::ptr::null_mut())};
        
        if err != 0 {
            panic!("fi_tx_context_failed {}", err);
        }

        Self { c_ep }
    }

    pub fn rx_context_with_context<T0>(&self, idx: i32, mut rxattr: crate::RxAttr, context : &mut T0) -> Self {
        let mut c_ep: *mut libfabric_sys::fid_ep = std::ptr::null_mut();
        let c_ep_ptr: *mut *mut libfabric_sys::fid_ep = &mut c_ep;

        let err = unsafe {libfabric_sys::inlined_fi_rx_context(self.c_ep, idx, rxattr.get_mut(), c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void)};
        
        if err != 0 {
            panic!("fi_tx_context_failed {}", err);
        }

        Self { c_ep }
    }

    pub fn rx_size_left(&self) -> isize {
        unsafe {libfabric_sys::inlined_fi_rx_size_left(self.c_ep)}
    }

    pub fn tx_size_left(&self) -> isize {
        unsafe {libfabric_sys::inlined_fi_tx_size_left(self.c_ep)}
    }
    
    pub fn recv<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_recv(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, std::ptr::null_mut()) };
        ret
    }

    pub fn recv_with_context<T0,T1,T2>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address, context: &mut T2) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_recv(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, context as *mut T2 as *mut std::ffi::c_void) };
        ret
    }

    pub fn trecv<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address, tag: u64, ignore:u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_trecv(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, tag, ignore, std::ptr::null_mut()) };
        ret  
    }

    pub fn trecv_with_context<T0,T1,T2>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address, tag: u64, ignore:u64, context: &mut T2) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_trecv(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, tag, ignore, context as *mut T2 as *mut std::ffi::c_void) };
        ret  
    }

    pub fn read<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], src_addr: crate::Address, addr: u64,  key: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_read(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, src_addr, addr, key, std::ptr::null_mut()) };
        ret
    }

    pub fn read_with_context<T0,T1,T2>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], src_addr: crate::Address, addr: u64,  key: u64, context: &mut T2) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_read(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, src_addr, addr, key, context as *mut T2 as *mut std::ffi::c_void) };
        ret
    }

    pub fn recvv<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_recvv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, addr, std::ptr::null_mut()) };
        ret
    }

    pub fn recvv_with_context<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, addr: crate::Address, context: &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_recvv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, addr, context as *mut T1 as *mut std::ffi::c_void) };
        ret
    }

    pub fn readv<T0>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, src_addr: crate::Address, addr: u64, key: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_readv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, src_addr, addr, key, std::ptr::null_mut()) };
        ret 
    }
    
    pub fn readv_with_context<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, src_addr: crate::Address, addr: u64, key: u64, context : &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_readv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, src_addr, addr, key, context as *mut T1 as *mut std::ffi::c_void) };
        ret 
    }

    pub fn trecvv<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, src_addr: crate::Address, tag: u64, ignore:u64, context : &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_trecvv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, src_addr, tag, ignore, context as *mut T1 as *mut std::ffi::c_void) };
        ret   
    }

    pub fn recvmsg(&self, msg: &crate::Msg, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_recvmsg(self.c_ep, msg.c_msg, flags) };
        ret
    }

    pub fn trecvmsg(&self, msg: &crate::MsgTagged, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_trecvmsg(self.c_ep, msg.c_msg_tagged, flags) };
        ret
    }

    pub fn readmsg(&self, msg: &crate::MsgRma, flags: u64) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_readmsg(self.c_ep, msg.c_msg_rma, flags) };
        ret
    }

    pub fn send<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_send(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, std::ptr::null_mut()) };
        ret
    }

    pub fn tsend<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], addr: crate::Address, tag:u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tsend(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, addr, tag, std::ptr::null_mut()) };
        ret
    }

    pub fn tsendv<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, dest_addr: crate::Address, tag:u64, context : &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tsendv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, dest_addr, tag, context as *mut T1 as *mut std::ffi::c_void) };
        ret   
    }

    pub fn write<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], dest_addr: crate::Address, addr: u64, key:u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_write(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, dest_addr, addr, key, std::ptr::null_mut()) };
        ret   
    }

    pub fn writev<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize,  dest_addr: crate::Address, addr: u64, key:u64, context : &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_writev(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, dest_addr, addr, key, context as *mut T1 as *mut std::ffi::c_void) };
        ret   
    }

    pub fn sendv<T0,T1>(&self, iov: &crate::IoVec, desc: &mut [T0], count: usize, addr: crate::Address, context: &mut T1) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_sendv(self.c_ep, iov.get(), desc.as_mut_ptr() as *mut *mut std::ffi::c_void, count, addr, context as *mut T1 as *mut std::ffi::c_void) };
        ret
    }

    pub fn sendmsg(&self, msg: &crate::Msg, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_sendmsg(self.c_ep, msg.c_msg, flags) };
        ret
    }

    pub fn tsendmsg(&self, msg: &crate::MsgTagged, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tsendmsg(self.c_ep, msg.c_msg_tagged, flags) };
        ret
    }

    pub fn writemsg(&self, msg: &crate::MsgRma, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_writemsg(self.c_ep, msg.c_msg_rma, flags) };
        ret
    }

    pub fn inject<T0>(&self, buf: &mut [T0], len: usize, addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_inject(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, addr) };
        ret
    }

    pub fn tinject<T0>(&self, buf: &mut [T0], len: usize, addr: crate::Address, tag:u64 ) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tinject(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, addr, tag) };
        ret
    }

    pub fn inject_write<T0>(&self, buf: &mut [T0], len: usize, dest_addr: crate::Address, addr: u64, key:u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_inject_write(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, dest_addr, addr, key) };
        ret
    }     

    pub fn senddata<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], data: u64, addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_senddata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, data, addr, std::ptr::null_mut()) };
        ret
    }

    pub fn tsenddata<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], data: u64, addr: crate::Address, tag: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tsenddata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, data, addr, tag, std::ptr::null_mut()) };
        ret
    }

    pub fn writedata<T0,T1>(&self, buf: &mut [T0], len: usize, desc: &mut [T1], data: u64, addr: crate::Address, other_addr: u64, key: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_writedata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, desc.as_mut_ptr() as *mut std::ffi::c_void, data, addr, other_addr, key, std::ptr::null_mut()) };
        ret
    }

    pub fn injectdata<T0>(&self, buf: &mut [T0], len: usize, data: u64, addr: crate::Address) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_injectdata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, data, addr) };
        ret
    }

    pub fn tinjectdata<T0>(&self, buf: &mut [T0], len: usize, data: u64, addr: crate::Address, tag: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_tinjectdata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, data, addr, tag) };
        ret
    }

    pub fn inject_writedata<T0>(&self, buf: &mut [T0], len: usize, data: u64, dest_addr: crate::Address, addr: u64, key: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_inject_writedata(self.c_ep, buf.as_mut_ptr() as *mut std::ffi::c_void, len, data, dest_addr, addr, key) };
        ret
    }

    pub fn getpeer<T0>(&self, addr: &mut [T0]) -> usize { //[TODO] Return result
        let mut len = addr.len();
        let len_ptr: *mut usize = &mut len;
        let _ = unsafe { libfabric_sys::inlined_fi_getpeer(self.c_ep, addr.as_mut_ptr() as *mut std::ffi::c_void, len_ptr)};
        
        len
    }

    pub fn connect<T0,T1>(&self, addr: & [T0], param: &[T1]) {
        let ret = unsafe { libfabric_sys::inlined_fi_connect(self.c_ep, addr.as_ptr() as *const std::ffi::c_void, param.as_ptr() as *const std::ffi::c_void, param.len()) };
        
        if ret != 0 {
            panic!("fi_connect failed {}", ret);
        }
    }

    pub fn accept<T0>(&self, param: &[T0]) {
        let ret = unsafe { libfabric_sys::inlined_fi_accept(self.c_ep, param.as_ptr() as *const std::ffi::c_void, param.len()) };
        
        if ret != 0 {
            panic!("fi_connect failed {}", ret);
        }
    }

    pub fn shutdown(&self, flags: u64) {
        let ret = unsafe { libfabric_sys::inlined_fi_shutdown(self.c_ep, flags) };

        if ret != 0 {
            panic!("fi_shutdown failed {}", ret);
        }
    }

    pub fn atomic<T0,T1>(&self, buf: &mut [T0], count : usize, desc: &mut T1, dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_atomic(self.c_ep, buf.as_mut_ptr()  as *mut std::ffi::c_void, count, desc as *mut T1  as *mut std::ffi::c_void, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        ret
    }

    pub fn atomicv<T0,T1>(&self, iov: &crate::Ioc, desc: &mut [T1], count : usize, dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_atomicv(self.c_ep, iov.get(), desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, count, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        ret
    }

    pub fn atomicmsg(&self, msg: &crate::MsgAtomic, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_atomicmsg(self.c_ep, msg.c_msg_atomic, flags) };
        ret
    }

    pub fn inject_atomic<T0,T1>(&self, buf: &mut [T0], count : usize, dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_inject_atomic(self.c_ep, buf.as_mut_ptr()  as *mut std::ffi::c_void, count, dest_addr, addr, key, datatype, op.get_value())};
        ret
    }

    pub fn fetch_atomic<T0,T1>(&self, buf: &mut [T0], count : usize, desc: &mut [T1], res: &mut [T0], res_desc: &mut [T1], dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_fetch_atomic(self.c_ep, buf.as_mut_ptr()  as *mut std::ffi::c_void, count, desc.as_mut_ptr()  as *mut std::ffi::c_void, res.as_mut_ptr()  as *mut std::ffi::c_void, res_desc.as_mut_ptr() as *mut std::ffi::c_void, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        ret
    }


    pub fn fetch_atomicv<T0,T1>(&self, iov: &crate::Ioc, desc: &mut [T1], count : usize, resultv: &mut crate::Ioc,  res_desc: &mut [T1], res_count : usize, dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize{
        let ret = unsafe{ libfabric_sys::inlined_fi_fetch_atomicv(self.c_ep, iov.get(), desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, count, resultv.get_mut(), res_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, res_count, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        ret
    }

    pub fn fetch_atomicmsg<T0>(&self, msg: &crate::MsgAtomic,  resultv: &mut crate::Ioc,  res_desc: &mut [T0], res_count : usize, flags: u64) -> isize {
        let ret = unsafe{ libfabric_sys::inlined_fi_fetch_atomicmsg(self.c_ep, msg.c_msg_atomic, resultv.get_mut(), res_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, res_count, flags) };
        ret
    }

    pub fn compare_atomic<T0, T1>(&self, buf: &mut [T0], count : usize, desc: &mut [T1], compare: &mut [T0], compare_desc: &mut [T1], 
            result: &mut [T0], result_desc: &mut [T1], dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize {
        let ret = unsafe {libfabric_sys::inlined_fi_compare_atomic(self.c_ep, buf.as_mut_ptr()  as *mut std::ffi::c_void, count, desc.as_mut_ptr()  as *mut std::ffi::c_void, compare.as_mut_ptr()  as *mut std::ffi::c_void, 
            compare_desc.as_mut_ptr()  as *mut std::ffi::c_void, result.as_mut_ptr()  as *mut std::ffi::c_void, result_desc.as_mut_ptr()  as *mut std::ffi::c_void, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        
        ret
    }

    pub fn compare_atomicv<T0>(&self, iov: &crate::Ioc, desc: &mut [T0], count : usize, comparetv: &mut crate::Ioc,  compare_desc: &mut [T0], compare_count : usize, 
        resultv: &mut crate::Ioc,  res_desc: &mut [T0], res_count : usize, dest_addr: Address, addr: u64, key: u64, datatype: crate::DataType, op: crate::enums::Op) -> isize {
        let ret = unsafe {libfabric_sys::inlined_fi_compare_atomicv(self.c_ep, iov.get(), desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, count, comparetv.get_mut(), compare_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, compare_count, resultv.get_mut(), res_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, res_count, dest_addr, addr, key, datatype, op.get_value(), std::ptr::null_mut())};
        
        ret
    }

    pub fn compare_atomicmsg<T0>(&self, msg: &crate::MsgAtomic, comparev: &crate::Ioc, compare_desc: &mut [T0], compare_count : usize, resultv: &mut crate::Ioc,  res_desc: &mut [T0], res_count : usize, flags: u64) -> isize {
        let res: isize = unsafe { libfabric_sys::inlined_fi_compare_atomicmsg(self.c_ep, msg.c_msg_atomic, comparev.get(), compare_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, compare_count, resultv.get_mut(), res_desc.as_mut_ptr()  as *mut *mut std::ffi::c_void, res_count, flags) };

        res
    }

    pub fn atomicvalid(&self, datatype: crate::DataType, op: crate::enums::Op) -> usize {
        let mut count: usize = 0;
        let err = unsafe { libfabric_sys:: inlined_fi_atomicvalid(self.c_ep, datatype, op.get_value(), &mut count as *mut usize)};

        if err != 0 {
            panic!("fi_atomicvalid failed {}", err);
        }

        count
    }

    pub fn fetch_atomicvalid(&self, datatype: crate::DataType, op: crate::enums::Op) -> usize {
        let mut count: usize = 0;
        let err = unsafe { libfabric_sys:: inlined_fi_fetch_atomicvalid(self.c_ep, datatype, op.get_value(), &mut count as *mut usize)};

        if err != 0 {
            panic!("fi_fetch_atomicvalid failed {}", err);
        }

        count
    }

    pub fn compare_atomicvalid(&self, datatype: crate::DataType, op: crate::enums::Op) -> usize {
        let mut count: usize = 0;
        let err = unsafe { libfabric_sys:: inlined_fi_compare_atomicvalid(self.c_ep, datatype, op.get_value(), &mut count as *mut usize)};

        if err != 0 {
            panic!("fi_fetch_atomicvalid failed {}", err);
        }

        count
    }

    pub fn join<T0,T1>(&self, addr: &T0, flags: u64, context: &mut T1 ) -> crate::Mc {
        crate::Mc::new(self, addr, flags, context)
    }

    pub fn join_collective<T0>(&self, coll_addr: Address, set: &crate::AvSet, flags: u64, context : &mut T0) -> crate::Mc {
        crate::Mc::new_collective(self, coll_addr, set, flags, context)
    }

    pub fn barrier<T0>(&self, addr: Address, context: &mut T0) -> isize {
        unsafe { libfabric_sys::inlined_fi_barrier(self.c_ep, addr, context as *mut T0 as *mut std::ffi::c_void) }
    }

    pub fn barrier2<T0>(&self, addr: Address, flags: u64, context: &mut T0) -> isize {
        unsafe { libfabric_sys::inlined_fi_barrier2(self.c_ep, addr, flags, context as *mut T0 as *mut std::ffi::c_void) }
    }

    pub fn broadcast<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, coll_addr: Address, root_addr: Address, datatype: DataType, flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_broadcast(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, coll_addr, root_addr, datatype, flags, context as *mut T2 as *mut std::ffi::c_void) }
    }

    pub fn alltoall<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, datatype: DataType, flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_alltoall(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, datatype, flags, context as *mut T2 as *mut std::ffi::c_void) }
    }

    pub fn allreduce<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, datatype: DataType, op: crate::enums::Op,  flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_allreduce(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, datatype, op.get_value(), flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
    
    pub fn allgather<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, datatype: DataType, flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_allgather(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, datatype, flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
    
    pub fn reduce_scatter<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, datatype: DataType, op: crate::enums::Op,  flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_reduce_scatter(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, datatype, op.get_value(), flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
    
    pub fn reduce<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, root_addr: Address, datatype: DataType, op: crate::enums::Op,  flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_reduce(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, root_addr, datatype, op.get_value(), flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
    
    pub fn scatter<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, root_addr: Address, datatype: DataType,  flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_scatter(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, root_addr, datatype, flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
    
    pub fn gather<T0,T1,T2>(&self, buf: &mut [T0], desc: &mut T1, result: &mut T0, result_desc: &mut T1, coll_addr: Address, root_addr: Address, datatype: DataType,  flags: u64, context: &mut T2) -> isize {
        unsafe { libfabric_sys::inlined_fi_gather(self.c_ep, buf as *mut [T0] as *mut std::ffi::c_void, buf.len(), desc as *mut T1 as *mut std::ffi::c_void, result as *mut T0 as *mut c_void, result_desc as *mut T1 as *mut c_void, coll_addr, root_addr, datatype, flags, context as *mut T2 as *mut std::ffi::c_void) }
    }
}

impl FID for Endpoint {
    fn fid(&self) -> *mut libfabric_sys::fid {
        unsafe { &mut (*self.c_ep).fid }
    }
}


pub struct Endpoint {
    pub(crate) c_ep: *mut libfabric_sys::fid_ep,
}

