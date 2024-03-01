

use debug_print::debug_println;

//================== Fabric (fi_fabric) ==================//
#[allow(unused_imports)]
use crate::AsFid;

pub struct Fabric {
    pub(crate) c_fabric: *mut libfabric_sys::fid_fabric,
    fid: crate::OwnedFid,
}


impl Fabric {
    pub fn new(mut attr: FabricAttr) -> Result<Fabric, crate::error::Error> {
        let mut c_fabric: *mut libfabric_sys::fid_fabric  = std::ptr::null_mut();
        let c_fabric_ptr: *mut *mut libfabric_sys::fid_fabric = &mut c_fabric;

        let err = unsafe {libfabric_sys::fi_fabric(attr.get_mut(), c_fabric_ptr, std::ptr::null_mut())};
        
        if err != 0 || c_fabric.is_null() {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self { c_fabric, fid: crate::OwnedFid { fid: unsafe{ &mut (*c_fabric).fid } } }
            )
        }
    }

    pub fn domain(&self, info: &crate::InfoEntry) -> Result<crate::domain::Domain, crate::error::Error> {
        crate::domain::Domain::new(self, info)
    } 

    pub fn domain2(&self, info: &crate::InfoEntry, flags: u64) -> Result<crate::domain::Domain, crate::error::Error> {
        crate::domain::Domain::new2(self, info, flags)
    }

    pub fn passive_ep(&self, info: &crate::InfoEntry) -> Result<crate::ep::PassiveEndpoint, crate::error::Error> {
        crate::ep::PassiveEndpoint::new(self, info)
    }

    // pub fn wait_open(&self, wait_attr: crate::sync::WaitAttr) -> Result<crate::sync::WaitSet, crate::error::Error> {
    //     crate::sync::WaitSet::new(self, wait_attr)
    // }

    pub fn trywait(&self, fids: &[&impl crate::AsFid]) -> Result<(), crate::error::Error> { // [TODO] Move this into the WaitSet struct
        let mut raw_fids: Vec<*mut libfabric_sys::fid> = fids.iter().map(|x| x.as_fid()).collect();
        let err = unsafe { libfabric_sys::inlined_fi_trywait(self.c_fabric, raw_fids.as_mut_ptr(), raw_fids.len() as i32) } ;
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }
}


impl crate::AsFid for Fabric {
    fn as_fid(&self) -> *mut libfabric_sys::fid {
        self.fid.as_fid()
    }
}


//================== Fabric attribute ==================//

#[derive(Clone, Debug)]
pub struct FabricAttr {
    c_attr : libfabric_sys::fi_fabric_attr,
}

impl FabricAttr {

    pub fn new() -> Self {
        let c_attr = libfabric_sys::fi_fabric_attr {
            fabric: std::ptr::null_mut(),
            name: std::ptr::null_mut(),
            prov_name: std::ptr::null_mut(),
            prov_version: 0,
            api_version: 0,
        };

        Self { c_attr }
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self) -> *const libfabric_sys::fi_fabric_attr {
        &self.c_attr
    }

    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_fabric_attr {
        &mut self.c_attr
    }

    pub fn get_prov_name(&self) -> String {
        unsafe{ std::ffi::CStr::from_ptr(self.c_attr.prov_name).to_str().unwrap().to_string() }
    }    

    pub fn get_name(&self) -> String {
        unsafe{ std::ffi::CStr::from_ptr(self.c_attr.name).to_str().unwrap().to_string() }
    }    
}

impl Default for FabricAttr {
    fn default() -> Self {
        Self::new()
    }
}
