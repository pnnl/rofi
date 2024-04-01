use crate::check_error;
use crate::ep::Endpoint;
use crate::ep::ActiveEndpoint;
use crate::infocapsoptions::RmaCap;
use crate::xcontext::ReceiveContext;
use crate::xcontext::TransmitContext;


impl<E: RmaCap> Endpoint<E> {

    pub unsafe fn read<T0>(&self, buf: &mut [T0], desc: &mut impl crate::DataDescriptor, src_addr: crate::Address, addr: u64,  key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_read(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), src_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn read_with_context<T0>(&self, buf: &mut [T0], desc: &mut impl crate::DataDescriptor, src_addr: crate::Address, addr: u64,  key: u64, context: &mut crate::Context) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_read(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), src_addr, addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }
    
    pub unsafe fn readv<T>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], src_addr: crate::Address, addr: u64, key: u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_readv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe  fn readv_with_context<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], src_addr: crate::Address, addr: u64, key: u64, context: &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_readv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, addr, key, (context as *mut T0).cast()) };

        check_error(err)
    }
    
    
    pub unsafe fn readmsg(&self, msg: &crate::MsgRma, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_readmsg(self.handle(), &msg.c_msg_rma as *const libfabric_sys::fi_msg_rma, flags) };
        
        check_error(err)
    }
    
    
    pub unsafe fn write<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error>  {
        let err = unsafe{ libfabric_sys::inlined_fi_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), dest_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn write_with_context<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, dest_addr: crate::Address, addr: u64, key:u64, context: &mut crate::Context) -> Result<(), crate::error::Error>  {
        let err = unsafe{ libfabric_sys::inlined_fi_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), dest_addr, addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }
    
    #[allow(unused_variables)]
    pub unsafe fn writev<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_writev(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, addr, key, std::ptr::null_mut()) };

        check_error(err)
    }

    #[allow(unused_variables)]
    pub unsafe fn writev_with_context<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], dest_addr: crate::Address, addr: u64, key:u64, context: &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_writev(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, addr, key, (context as *mut T0).cast()) };

        check_error(err)
    }
    
    pub unsafe fn writemsg(&self, msg: &crate::MsgRma, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writemsg(self.handle(), &msg.c_msg_rma as *const libfabric_sys::fi_msg_rma, flags) };
        
        check_error(err)
    }
    
    pub unsafe fn writedata<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, data: u64, addr: crate::Address, other_addr: u64, key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, other_addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn writedata_with_context<T0,T1>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, data: u64, addr: crate::Address, other_addr: u64, key: u64, context: &mut crate::Context) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, other_addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }

    pub unsafe fn inject_write<T0>(&self, buf: &[T0], dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_inject_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), dest_addr, addr, key) };
    
        check_error(err)
    }     

    pub unsafe fn inject_writedata<T0>(&self, buf: &[T0], data: u64, dest_addr: crate::Address, addr: u64, key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_inject_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), data, dest_addr, addr, key) };
    
        check_error(err)
    }
}

impl TransmitContext {

    pub unsafe fn write<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error>  {
        let err = unsafe{ libfabric_sys::inlined_fi_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), dest_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn write_with_context<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, dest_addr: crate::Address, addr: u64, key:u64, context: &mut crate::Context) -> Result<(), crate::error::Error>  {
        let err = unsafe{ libfabric_sys::inlined_fi_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), dest_addr, addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }
    
    pub unsafe fn writev<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_writev(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn writev_with_context<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], dest_addr: crate::Address, addr: u64, key:u64, context: &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_writev(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, addr, key, (context as *mut T0).cast()) };
        
        check_error(err)
    }
    
    pub unsafe fn writemsg(&self, msg: &crate::MsgRma, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writemsg(self.handle(), &msg.c_msg_rma as *const libfabric_sys::fi_msg_rma, flags) };
        
        check_error(err)
    }
    
    pub unsafe fn writedata<T0>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, data: u64, addr: crate::Address, other_addr: u64, key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, other_addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn writedata_with_context<T0,T1>(&self, buf: &[T0], desc: &mut impl crate::DataDescriptor, data: u64, addr: crate::Address, other_addr: u64, key: u64, context: &mut crate::Context) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, other_addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }

    pub unsafe fn inject_write<T0>(&self, buf: &[T0], dest_addr: crate::Address, addr: u64, key:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_inject_write(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), dest_addr, addr, key) };
    
        check_error(err)
    }     

    pub unsafe fn inject_writedata<T0>(&self, buf: &[T0], data: u64, dest_addr: crate::Address, addr: u64, key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_inject_writedata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), data, dest_addr, addr, key) };
    
        check_error(err)
    }
}

impl ReceiveContext {

    pub unsafe fn read<T0>(&self, buf: &mut [T0], desc: &mut impl crate::DataDescriptor, src_addr: crate::Address, addr: u64,  key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_read(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), src_addr, addr, key, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub unsafe fn read_with_context<T0>(&self, buf: &mut [T0], desc: &mut impl crate::DataDescriptor, src_addr: crate::Address, addr: u64,  key: u64, context: &mut crate::Context) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_read(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), src_addr, addr, key, context.get_mut() as *mut  std::ffi::c_void) };
        
        check_error(err)
    }
    
    pub unsafe fn readv<T>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], src_addr: crate::Address, addr: u64, key: u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_readv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, addr, key, std::ptr::null_mut()) };

        check_error(err)
    }
    
    pub unsafe  fn readv_with_context<T, T0>(&self, iov: &[crate::IoVec<T>], desc: &mut [impl crate::DataDescriptor], src_addr: crate::Address, addr: u64, key: u64, context: &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_readv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, addr, key, (context as *mut T0).cast()) };

        check_error(err)
    }
    
    
    pub unsafe fn readmsg(&self, msg: &crate::MsgRma, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_readmsg(self.handle(), &msg.c_msg_rma as *const libfabric_sys::fi_msg_rma, flags) };
        
        check_error(err)
    }
}


