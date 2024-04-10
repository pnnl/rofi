use crate::ep::Endpoint;
use crate::ep::ActiveEndpoint;
use crate::infocapsoptions::RecvMod;
use crate::infocapsoptions::SendMod;
use crate::infocapsoptions::TagCap;
use crate::mr::DataDescriptor;
use crate::utils::check_error;
use crate::xcontext::ReceiveContext;
use crate::xcontext::TransmitContext;




impl<E: TagCap + RecvMod> Endpoint<E> {

    pub fn trecv<T>(&self, buf: &mut [T], desc: &mut impl DataDescriptor, addr: crate::Address, tag: u64, ignore:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecv(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, ignore, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub fn trecv_with_context<T, T0>(&self, buf: &mut [T], desc: &mut impl DataDescriptor, addr: crate::Address, tag: u64, ignore:u64, context: &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecv(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, ignore, (context as *mut T0).cast()) };
        
        check_error(err)
    }

	pub fn trecvv<T, T0>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], src_addr: crate::Address, tag: u64, ignore:u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_trecvv(self.handle(), iov.as_ptr().cast() , desc.as_mut_ptr().cast(), iov.len(), src_addr, tag, ignore, std::ptr::null_mut()) };

        check_error(err)   
    }

	pub fn trecvv_with_context<T, T0>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], src_addr: crate::Address, tag: u64, ignore:u64, context : &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_trecvv(self.handle(), iov.as_ptr().cast() , desc.as_mut_ptr().cast(), iov.len(), src_addr, tag, ignore, (context as *mut T0).cast()) };

        check_error(err)   
    }

    pub fn trecvmsg(&self, msg: &crate::msg::MsgTagged, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecvmsg(self.handle(), &msg.c_msg_tagged as *const libfabric_sys::fi_msg_tagged, flags) };
    
        check_error(err)
    }
}

impl<E: TagCap + SendMod> Endpoint<E> {

    pub fn tsend<T>(&self, buf: &[T], desc: &mut impl DataDescriptor, addr: crate::Address, tag:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsend(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, std::ptr::null_mut()) };
    
        check_error(err)
    }

    pub fn tsend_with_context<T, T0>(&self, buf: &[T], desc: &mut impl DataDescriptor, addr: crate::Address, tag:u64, context : &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsend(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, (context as *mut T0).cast()) };
    
        check_error(err)
    }
    
	pub fn tsendv<T>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], dest_addr: crate::Address, tag:u64) -> Result<(), crate::error::Error> { // [TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_tsendv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, tag, std::ptr::null_mut()) };

        check_error(err)
    }

	pub fn tsendv_with_context<T, T0>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], dest_addr: crate::Address, tag:u64, context : &mut T0) -> Result<(), crate::error::Error> { // [TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_tsendv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, tag, (context as *mut T0).cast()) };

        check_error(err)
    }

    pub fn tsendmsg(&self, msg: &crate::msg::MsgTagged, flags: crate::enums::TransferOptions) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsendmsg(self.handle(), &msg.c_msg_tagged as *const libfabric_sys::fi_msg_tagged, flags.get_value().into()) };
    
        check_error(err)
    }

    pub fn tsenddata<T>(&self, buf: &[T], desc: &mut impl DataDescriptor, data: u64, addr: crate::Address, tag: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsenddata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, tag, std::ptr::null_mut()) };
    
        check_error(err)
    }

    pub fn tsenddata_with_context<T, T0>(&self, buf: &[T], desc: &mut impl DataDescriptor, data: u64, addr: crate::Address, tag: u64, context : &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsenddata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, tag, (context as *mut T0).cast()) };
    
        check_error(err)
    }

    pub fn tinject<T>(&self, buf: &[T], addr: crate::Address, tag:u64 ) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tinject(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), addr, tag) };
    
        check_error(err)
    }

    pub fn tinjectdata<T>(&self, buf: &[T], data: u64, addr: crate::Address, tag: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tinjectdata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), data, addr, tag) };
    
        check_error(err)
    }
}


impl TransmitContext {

    pub fn tsend<T>(&self, buf: &[T], desc: &mut impl DataDescriptor, addr: crate::Address, tag:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsend(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, std::ptr::null_mut()) };
    
        check_error(err)
    }

    pub fn tsend_with_context<T, T0>(&self, buf: &[T], desc: &mut impl DataDescriptor, addr: crate::Address, tag:u64, context : &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsend(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, (context as *mut T0).cast()) };
    
        check_error(err)
    }
    
	pub fn tsendv<T>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], dest_addr: crate::Address, tag:u64) -> Result<(), crate::error::Error> { // [TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_tsendv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, tag, std::ptr::null_mut() ) };

        check_error(err)
    }

	pub fn tsendv_with_context<T, T0>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], dest_addr: crate::Address, tag:u64, context : &mut T0) -> Result<(), crate::error::Error> { // [TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_tsendv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), dest_addr, tag, (context as *mut T0).cast() ) };

        check_error(err)
    }

    pub fn tsendmsg(&self, msg: &crate::msg::MsgTagged, flags: crate::enums::TransferOptions) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsendmsg(self.handle(), &msg.c_msg_tagged as *const libfabric_sys::fi_msg_tagged, flags.get_value().into()) };
    
        check_error(err)
    }

    pub fn tsenddata<T>(&self, buf: &[T], desc: &mut impl DataDescriptor, data: u64, addr: crate::Address, tag: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsenddata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, tag, std::ptr::null_mut()) };
    
        check_error(err)
    }

    pub fn tsenddata_with_context<T, T0>(&self, buf: &[T], desc: &mut impl DataDescriptor, data: u64, addr: crate::Address, tag: u64, context : &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tsenddata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), data, addr, tag, (context as *mut T0).cast()) };
    
        check_error(err)
    }

    pub fn tinject<T>(&self, buf: &[T], addr: crate::Address, tag:u64 ) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tinject(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), addr, tag) };
    
        check_error(err)
    }

    pub fn tinjectdata<T>(&self, buf: &[T], data: u64, addr: crate::Address, tag: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_tinjectdata(self.handle(), buf.as_ptr() as *const std::ffi::c_void, std::mem::size_of_val(buf), data, addr, tag) };
    
        check_error(err)
    }
}

impl ReceiveContext {

    pub fn trecv<T>(&self, buf: &mut [T], desc: &mut impl DataDescriptor, addr: crate::Address, tag: u64, ignore:u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecv(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, ignore, std::ptr::null_mut()) };
        
        check_error(err)
    }
    
    pub fn trecv_with_context<T, T0>(&self, buf: &mut [T], desc: &mut impl DataDescriptor, addr: crate::Address, tag: u64, ignore:u64, context: &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecv(self.handle(), buf.as_mut_ptr() as *mut std::ffi::c_void, std::mem::size_of_val(buf), desc.get_desc(), addr, tag, ignore, (context as *mut T0).cast()) };
        
        check_error(err)
    }

    pub fn trecvv<T>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], src_addr: crate::Address, tag: u64, ignore:u64) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_trecvv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, tag, ignore, std::ptr::null_mut()) };
    
        check_error(err)
    }

	pub fn trecvv_with_context<T, T0>(&self, iov: &[crate::iovec::IoVec<T>], desc: &mut [impl DataDescriptor], src_addr: crate::Address, tag: u64, ignore:u64, context : &mut T0) -> Result<(), crate::error::Error> { //[TODO]
        let err = unsafe{ libfabric_sys::inlined_fi_trecvv(self.handle(), iov.as_ptr().cast(), desc.as_mut_ptr().cast(), iov.len(), src_addr, tag, ignore, (context as *mut T0).cast()) };
    
        check_error(err)
    }

    pub fn trecvmsg(&self, msg: &crate::msg::MsgTagged, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_trecvmsg(self.handle(), &msg.c_msg_tagged as *const libfabric_sys::fi_msg_tagged, flags) };
    
        check_error(err)
    }
}
