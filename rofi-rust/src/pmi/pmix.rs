use crate::pmi;
use pmi::{PmiTrait,ErrorKind,Error};
use core::panic;
use std::collections::HashMap;

use debug_print::debug_println;

extern crate pmix_sys;

pub struct PmiX {
    rank: usize,
    size: usize,
    nspace: pmix_sys::pmix_nspace_t,
    finalize: bool,
    names: HashMap<String, usize>,
    singleton_kvs: HashMap<String, Vec<u8> >,
}

impl PmiX {
    
    pub(crate) fn init() -> Result<Self, Error> {
        
        let finalize;
        let mut proc = pmix_sys::pmix_proc {
            nspace: [0; 256usize],
            rank: 0,
        };

        let err = unsafe { pmix_sys::PMIx_Init(&mut proc, std::ptr::null_mut(), 0)} ;
        let rank = proc.rank;
        if pmix_sys::PMIX_SUCCESS != err as u32 {
            return Err(Error::from_pmix_err_code(err));
        }
        finalize = true;
        proc.rank = pmix_sys::PMIX_RANK_WILDCARD;
        let key = pmix_sys::PMIX_JOB_SIZE;
        let mut val: *mut pmix_sys::pmix_value_t = std::ptr::null_mut();
        let err = unsafe {pmix_sys::PMIx_Get(&proc, key.as_ptr() as *mut i8, std::ptr::null(), 0, &mut val)};
        if pmix_sys::PMIX_SUCCESS != err as u32 {
            return Err(Error::from_pmix_err_code(err));
        }

        let size = unsafe{(*val).data.uint32};


        debug_println!("Found rank: {}", rank);
        
        Ok(PmiX {
            rank: rank as usize,
            size: size as usize,
            nspace: proc.nspace,
            finalize,
            names: HashMap::new(),
            singleton_kvs: HashMap::new(),
        })
    }

    pub(crate) fn get_rank(&self) -> usize {
        
        self.rank
    }


    pub(crate) fn get_size(&self) -> usize {

        self.size
    }

    fn get_singleton(&self, key: &str) -> Result<Vec<u8>, Error> {
       
        if let Some(data) = self.singleton_kvs.get(key) {
            Ok(data.clone())
        }
        else {
            Err(Error{c_err: pmix_sys::PMIX_ERR_INVALID_KEY as i32, kind: ErrorKind::InvalidKey})
        }
    }

    fn put_singleton(&mut self, key: &str, value: &[u8]) {
        
        self.singleton_kvs.insert(key.to_owned(), value.to_vec());
    }

    pub(crate) fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        if self.size > 1 {
            let mut val: pmix_sys::pmix_value = pmix_sys::pmix_value {
                type_ : 0,
                data: pmix_sys::pmix_value__bindgen_ty_1{
                    string: std::ptr::null_mut(),
                },
            };
            let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",self.rank,key)).unwrap().into_raw();
            let mut kvs_val = self.encode(value);
            
            unsafe{pmix_sys::pmix_value_load(&mut val, kvs_val.as_mut_ptr() as *mut std::ffi::c_void,  pmix_sys::PMIX_STRING as u16)};
            let err = unsafe { pmix_sys::PMIx_Put(pmix_sys::PMIX_GLOBAL as u8, kvs_key, &mut val) } as u32;
            if pmix_sys::PMIX_SUCCESS != err as u32 { 
                return Err(Error::from_pmix_err_code(err as i32));
            }
            let err = unsafe{ pmix_sys::PMIx_Commit()};
            if pmix_sys::PMIX_SUCCESS != err  as u32{ 
                return Err(Error::from_pmix_err_code(err as i32));
            }
        }
        else {
            self.put_singleton(key, value);
        }
        Ok(())
    }

    pub(crate) fn get(&self, rank: usize, key: &str, _len: usize) -> Result<Vec<u8>, Error>{
        if self.size > 1 {
            let proc = pmix_sys::pmix_proc {
                nspace: self.nspace.clone(),
                rank: rank as u32,
            };
            let mut recv_val: *mut pmix_sys::pmix_value = std::ptr::null_mut();
            let mut val: pmix_sys::pmix_value = pmix_sys::pmix_value {
                type_ : 0,
                data: pmix_sys::pmix_value__bindgen_ty_1{
                    string: std::ptr::null_mut(),
                },
            };

            let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",rank,key)).unwrap().into_raw();
            
            let err = unsafe { pmix_sys::PMIx_Get(&proc, kvs_key, std::ptr::null(), 0, &mut recv_val) };
            if pmix_sys::PMIX_SUCCESS != err as u32 {
                return Err(Error::from_pmix_err_code(err));
            }
            unsafe{pmix_sys::pmix_value_xfer(&mut val, recv_val)};

            let byte_array = unsafe {std::ffi::CStr::from_ptr(val.data.string)}.to_bytes_with_nul();
            

            Ok(self.decode(&byte_array))
        }
        else {
            self.get_singleton(key)
        }
    }

    pub(crate) fn exchange_data(&mut self, key: &str, value: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        
        let res = self.names.get_mut(key);
        let new_key = key.to_owned() + & match res {
            Some(val) => {*val+=1; val.clone()},
            None => {self.names.insert(key.to_owned(), 0); 0}
        }.to_string();

        self.put(&new_key, value)?;

        self.exchange()?;

        let mut res = Vec::new();
        for i in 0..self.size as usize {
            let ret = self.get(i, &new_key, value.len())?;
            res.push(ret);
        }

        Ok(res)
    }

    pub(crate) fn exchange(&self) -> Result<(), Error> {
        if self.size > 1 {

            let err = unsafe{ pmix_sys::PMIx_Commit()};
            if pmix_sys::PMIX_SUCCESS != err  as u32{ 
                return Err(Error::from_pmix_err_code(err as i32));
            }
            // let err = unsafe{ pmix_sys::PMIx_Fence(std::ptr::null_mut(), 0, std::ptr::null_mut(), 0) };
            // if err as u32 != pmix_sys::PMIX_SUCCESS {
            //     return Err(Error::from_pmix_err_code(err));
            // }
        }

        Ok(())
    }
    
    // PMIX has no barrier call, so we use KVS_Fence for global synchronization
    // Since implementations might do it lazily we also put and get a small buffer to 
    // force the synchronization to happen.
    // pub(crate) fn barrier(&self) -> Result<(), Error> {
    //     let data = 0;
    //     self.put("barrier", std::slice::from_ref(&data))?;
        
    //     self.exchange()?;

    //     let data_back = self.get((self.rank + 1) % self.size, "barrier", 1)?;

    //     Ok(())
    // }
}

impl Drop for PmiX {
    fn drop(&mut self) {
        if self.finalize {

            let err = unsafe{ pmix_sys::PMIx_Finalize(std::ptr::null(), 0) } as u32;
            if err != pmix_sys::PMIX_SUCCESS {
                panic!("Failed to finailize PMIX {}", err);
            }
        }
    }
}

impl PmiTrait for PmiX {
    fn get_rank(&self) -> usize {
        self.get_rank()
    }
    fn get_size(&self) -> usize {
        self.get_size()
    }
    
    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        self.put(key, value)
    }
    
    fn get(&self, rank: usize, key: &str, len: usize) -> Result<Vec<u8>, Error> {
        self.get(rank, key, len)
    }

    fn exchange(&self) -> Result<(), Error> {
        self.exchange()
    }

    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        
        self.exchange_data(key, value)
    }

    // fn barrier(&self) -> Result<(), Error> {

    //     self.barrier()
    // }
}

impl Error {
    pub(crate) fn from_pmix_err_code(c_err: i32) -> Self {

        let kind;
        if c_err == pmix_sys::PMIX_ERROR {
            kind = ErrorKind::OperationFailed;
        }
        else{
            kind = match c_err {
                pmix_sys::PMIX_ERR_INIT => ErrorKind::NotInitialized,
                pmix_sys::PMIX_ERR_NOMEM => ErrorKind::NoBufSpaceAvailable,
                pmix_sys::PMIX_ERR_INVALID_ARG => ErrorKind::InvalidArg,
                pmix_sys::PMIX_ERR_INVALID_KEY => ErrorKind::InvalidKey,
                pmix_sys::PMIX_ERR_INVALID_KEY_LENGTH => ErrorKind::InvalidKeyLength,
                pmix_sys::PMIX_ERR_INVALID_VAL => ErrorKind::InvalidVal,
                pmix_sys::PMIX_ERR_INVALID_VAL_LENGTH => ErrorKind::InvalidValLength,
                pmix_sys::PMIX_ERR_INVALID_LENGTH => ErrorKind::InvalidLength,
                pmix_sys::PMIX_ERR_INVALID_NUM_ARGS => ErrorKind::InvalidNumArgs,
                pmix_sys::PMIX_ERR_INVALID_ARGS => ErrorKind::InvalidArgs,
                pmix_sys::PMIX_ERR_INVALID_NUM_PARSED => ErrorKind::InvalidNumParsed,
                pmix_sys::PMIX_ERR_INVALID_KEYVALP => ErrorKind::InvalidKeyValP,
                pmix_sys::PMIX_ERR_INVALID_SIZE => ErrorKind::InvalidSize,
                _ => ErrorKind::Other,
            };
        }

        Self {c_err: c_err as i32, kind}
    }
}