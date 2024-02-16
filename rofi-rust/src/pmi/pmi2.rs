use crate::pmi;
use pmi::{PmiTrait,ErrorKind,Error};
use core::panic;
use std::collections::HashMap;

use debug_print::debug_println;

extern crate pmi2_sys;

pub struct Pmi2 {
    rank: usize,
    size: usize,
    finalize: bool,
    names: HashMap<String, usize>,
    singleton_kvs: HashMap<String, Vec<u8> >,
}

impl Pmi2 {
    
    pub(crate) fn init() -> Result<Self, Error> {
        
        let mut rank = 0;
        let mut size = 0;
        let mut spawned = 0;
        let mut appnum = 0;
        let finalize;

        if  unsafe{ pmi2_sys::PMI2_Initialized() } == 0 {
            let err = unsafe { pmi2_sys::PMI2_Init(&mut spawned as *mut i32, &mut size as *mut i32, &mut rank as *mut i32, &mut appnum as *mut i32)} ;

            if pmi2_sys::PMI2_SUCCESS != err as u32 {
                return Err(Error::from_pmi2_err_code(err));
            }
            finalize = true;
        }
        else {
            todo!()
        }

        debug_println!("Found rank: {}, size: {}, spawned: {}, appnum: {}", rank, size, spawned, appnum);
        
        Ok(Pmi2 {
            rank: rank as usize,
            size: size as usize,
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
            Err(Error{c_err: pmi2_sys::PMI2_ERR_INVALID_KEY as i32, kind: ErrorKind::InvalidKey})
        }
    }

    fn put_singleton(&mut self, key: &str, value: &[u8]) {
        
        self.singleton_kvs.insert(key.to_owned(), value.to_vec());
    }

    pub(crate) fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        if self.size > 1 {
            let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",self.rank,key)).unwrap().into_raw();
            let kvs_val = self.encode(value);

            let err = unsafe { pmi2_sys::PMI2_KVS_Put(kvs_key, kvs_val.as_ptr() as *const i8) } as u32;
            if pmi2_sys::PMI2_SUCCESS != err { 
                return Err(Error::from_pmi2_err_code(err as i32));
            }
        }
        else {
            self.put_singleton(key, value);
        }
        Ok(())
    }

    pub(crate) fn get(&self, rank: usize, key: &str, len: usize) -> Result<Vec<u8>, Error>{
        if self.size > 1 {
            let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",rank,key)).unwrap().into_raw();
            let mut kvs_val: Vec<u8> = vec![0; 2 * len + 1];
            let mut len = 0;
            let err = unsafe { pmi2_sys::PMI2_KVS_Get(std::ptr::null(), pmi2_sys::PMI2_ID_NULL ,kvs_key, kvs_val.as_mut_ptr() as *mut i8, kvs_val.len() as i32, &mut len as *mut i32) };
            if pmi2_sys::PMI2_SUCCESS != err as u32 {
                return Err(Error::from_pmi2_err_code(err));
            }

            Ok(self.decode(&kvs_val))
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
        
            let err = unsafe{ pmi2_sys::PMI2_KVS_Fence() };
            if err as u32 != pmi2_sys::PMI2_SUCCESS {
                return Err(Error::from_pmi2_err_code(err));
            }
        }

        Ok(())
    }
    
    // PMI2 has no barrier call, so we use KVS_Fence for global synchronization
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

impl Drop for Pmi2 {
    fn drop(&mut self) {
        if self.finalize {

            let err = unsafe{ pmi2_sys::PMI2_Finalize() } as u32;
            if err != pmi2_sys::PMI2_SUCCESS {
                panic!("Failed to finailize PMI2 {}", err);
            }
        }
    }
}

impl PmiTrait for Pmi2 {
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
    pub(crate) fn from_pmi2_err_code(c_err: i32) -> Self {

        let kind;
        if c_err == pmi2_sys::PMI2_FAIL {
            kind = ErrorKind::OperationFailed;
        }
        else{
            let c_err = c_err as u32;
            kind = match c_err {
                pmi2_sys::PMI2_ERR_INIT => ErrorKind::NotInitialized,
                pmi2_sys::PMI2_ERR_NOMEM => ErrorKind::NoBufSpaceAvailable,
                pmi2_sys::PMI2_ERR_INVALID_ARG => ErrorKind::InvalidArg,
                pmi2_sys::PMI2_ERR_INVALID_KEY => ErrorKind::InvalidKey,
                pmi2_sys::PMI2_ERR_INVALID_KEY_LENGTH => ErrorKind::InvalidKeyLength,
                pmi2_sys::PMI2_ERR_INVALID_VAL => ErrorKind::InvalidVal,
                pmi2_sys::PMI2_ERR_INVALID_VAL_LENGTH => ErrorKind::InvalidValLength,
                pmi2_sys::PMI2_ERR_INVALID_LENGTH => ErrorKind::InvalidLength,
                pmi2_sys::PMI2_ERR_INVALID_NUM_ARGS => ErrorKind::InvalidNumArgs,
                pmi2_sys::PMI2_ERR_INVALID_ARGS => ErrorKind::InvalidArgs,
                pmi2_sys::PMI2_ERR_INVALID_NUM_PARSED => ErrorKind::InvalidNumParsed,
                pmi2_sys::PMI2_ERR_INVALID_KEYVALP => ErrorKind::InvalidKeyValP,
                pmi2_sys::PMI2_ERR_INVALID_SIZE => ErrorKind::InvalidSize,
                _ => ErrorKind::Other,
            };
        }

        Self {c_err: c_err as i32, kind}
    }
}