use crate::pmi;
use crate::pmi::{PmiTrait, ErrorKind, Error};
use core::panic;
use std::{collections::HashMap, ffi::CString};

use debug_print::debug_println;


extern crate pmi_sys;

pub struct Pmi1 {
    rank: usize,
    size: usize,
    finalize: bool,
    kvs_name: std::ffi::CString,
    names: HashMap<String, usize>,
    singleton_kvs: HashMap<String, Vec<u8> >,
}


// pub struct PmiAsyncGet<'a> {
//     pmi: &'a Pmi1,
//     key: String,
//     rank: usize,
//     len: usize,
//     waker: std::cell::RefCell<Option<std::task::Waker>>,
// }

// impl<'a> async_std::future::Future for PmiAsyncGet<'a> {
//     type Output = Vec<u8>;

//     fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
//         let err = self.pmi.get(self.rank, &self.key, self.len);
//         match err {

//             Ok(data) => {
//                 std::task::Poll::Ready(data)
//             }
//             Err(error) => {
//                 if !matches!(error.kind, ErrorKind::InvalidKey) {
//                     panic!("Unexpected error in KVS get {}", error.c_err);
//                 }
//                 else {
//                     *self.waker.borrow_mut() = Some(cx.waker().clone());
//                     cx.waker().wake_by_ref();
                    
//                     std::task::Poll::Pending
//                 }
//             }
//         }
//     }
// }

impl Pmi1 {
    
    pub(crate) fn init() -> Result<Self, Error> {
        
        let mut rank = 0;
        let mut size = 0;
        let mut spawned = 0;
        let mut appnum = 0;
        let mut finalize = false;
        let mut init= 0;
        let mut max_name_len: i32 = 0;

        let err_code = unsafe { pmi_sys::PMI_Initialized(&mut init as *mut i32) };
        if  err_code as u32 == pmi_sys::PMI_SUCCESS {
            if init as u32 == pmi_sys::PMI_FALSE {
                let err = unsafe { pmi_sys::PMI_Init(&mut spawned as *mut i32)} as u32;
                if err != pmi_sys::PMI_SUCCESS {
                    panic!("Could not initialize PMI");
                }
                finalize = true;
            }
        }
        else {
            return Err(Error::from_pmi1_err_code(err_code as i32));
        }

        let err_code = unsafe{ pmi_sys::PMI_Get_size(&mut size as *mut i32)};
        if err_code as u32 != pmi_sys::PMI_SUCCESS {
           return Err(Error::from_pmi1_err_code(err_code as i32));
        }
        
        let err_code = unsafe{ pmi_sys::PMI_Get_rank(&mut rank as *mut i32)};
        if unsafe{ pmi_sys::PMI_Get_rank(&mut rank as *mut i32)} as u32 != pmi_sys::PMI_SUCCESS {
            return Err(Error::from_pmi1_err_code(err_code as i32));
        }
        
        let err_code = unsafe{ pmi_sys::PMI_Get_appnum(&mut appnum as *mut i32)};
        if err_code as u32 != pmi_sys::PMI_SUCCESS {
            return Err(Error::from_pmi1_err_code(err_code as i32));
        }
        
        let err_code = unsafe {pmi_sys::PMI_KVS_Get_name_length_max(&mut max_name_len as *mut i32)};
        if err_code as u32 != pmi_sys::PMI_SUCCESS {
            return Err(Error::from_pmi1_err_code(err_code as i32));
        }
        
        let mut name: Vec<u8> = vec![0; max_name_len as usize];
        let err_code = unsafe {pmi_sys::PMI_KVS_Get_my_name(name.as_mut_ptr() as *mut i8, max_name_len) };
        if err_code as u32 != pmi_sys::PMI_SUCCESS {
            return Err(Error::from_pmi1_err_code(err_code as i32));
        }
        
        let kvs_name = std::ffi::CString::new(name);
        let real_kvs_name = match kvs_name {
            Err(error) => {let len = error.nul_position(); std::ffi::CString::new(&error.into_vec()[..len]).unwrap() },
            Ok(real_name) => real_name,
        };

        debug_println!("Found rank: {}, size: {}, spawned: {}, appnum: {}", rank, size, spawned, appnum);
        
        Ok(Pmi1 {
            rank: rank as usize,
            size: size as usize,
            finalize,
            kvs_name: real_kvs_name,
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
            Err(Error{c_err: pmi_sys::PMI_ERR_INVALID_KEY as i32, kind: ErrorKind::InvalidKey})
        }
    }

    fn put_singleton(&mut self, key: &str, value: &[u8]) {
        
        self.singleton_kvs.insert(key.to_owned(), value.to_vec());
    }

    pub(crate) fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        if self.size > 1 {

            let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",self.rank,key)).unwrap().into_raw();
            let kvs_val = self.encode(value);
            
            let err = unsafe { pmi_sys::PMI_KVS_Put(self.kvs_name.as_ptr(), kvs_key, kvs_val.as_ptr() as *const i8) } as u32;
            if pmi_sys::PMI_SUCCESS != err { 
                return Err(Error::from_pmi1_err_code(err as i32));
            }
            
            let err = unsafe{ pmi_sys::PMI_KVS_Commit(self.kvs_name.as_ptr()) } as u32;
            if pmi_sys::PMI_SUCCESS != err { 
                return Err(Error::from_pmi1_err_code(err as i32 ));
            }
            
            #[allow(unused_variables)]
            let kvs_key_ = unsafe { CString::from_raw(kvs_key) };
            debug_println!("P[{}] Putting to KVS {:?}\n\tKey: {:?}\n\tValue: {:x?}", self.get_rank(), self.kvs_name, kvs_key_, value);
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
            
            let err = unsafe { pmi_sys::PMI_KVS_Get(self.kvs_name.as_ptr() ,kvs_key, kvs_val.as_mut_ptr() as *mut i8, kvs_val.len() as i32) };
            if pmi_sys::PMI_SUCCESS != err as u32 {
                return Err(Error::from_pmi1_err_code(err as i32));
            }
            
            let dec = self.decode(&kvs_val);
            #[allow(unused_variables)]
            let kvs_key_ = unsafe { CString::from_raw(kvs_key) };
            debug_println!("P[{}] Getting from KVS {:?}:\n\tKey: {:?}\n\tValue: {:x?}",  self.get_rank(), self.kvs_name, kvs_key_, dec);
            
            Ok(dec)
        }
        else {
            self.get_singleton(key)
        }
    }

    pub(crate) fn exchange_data(&mut self, key: &str, value: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        let res = self.names.get_mut(key);
        let new_key = key.to_owned() + & match res {
            Some(val) => {*val+=1; *val},
            None => {self.names.insert(key.to_owned(), 0); 0}
        }.to_string();


        self.put(&new_key, value)?;

        self.exchange()?;

        let mut res = Vec::new();
        for i in 0..self.size {
            let ret = self.get(i, &new_key, value.len())?;
            res.push(ret);
        }

        Ok(res)
    }

    pub(crate) fn exchange(&self) -> Result<(), Error> {
        
        if self.size > 1 {

            let err = unsafe{ pmi_sys::PMI_KVS_Commit(self.kvs_name.as_ptr()) } as u32;
            
            if err != pmi_sys::PMI_SUCCESS {
                return Err(Error::from_pmi1_err_code(err as i32));
            }

            let err = unsafe{ pmi_sys::PMI_Barrier() };
            if err as u32 != pmi_sys::PMI_SUCCESS {
                return Err(Error::from_pmi1_err_code(err as i32));
            }
        }

        Ok(())
    }

    // pub(crate) fn barrier(&self) -> Result<(), Error> {

    //     if self.size > 1 {

    //         let err = unsafe { pmi_sys::PMI_Barrier() } as u32 ;

    //         if err != pmi_sys::PMI_SUCCESS {
    //             return Err(Error::from_pmi1_err_code(err as i32));
    //         }
    //     }

    //     Ok(())
    // }

    
}

impl Drop for Pmi1 {
    fn drop(&mut self) {
        if self.finalize {
            let err = unsafe{ pmi_sys::PMI_Finalize() } as u32;
            if err != pmi_sys::PMI_SUCCESS {
                panic!("{:?}", Error::from_pmi1_err_code(err as i32 ))
            }
        }
    }
}

impl pmi::PmiTrait for Pmi1 {
    fn get_rank(&self) -> usize {
        self.get_rank()
    }
    fn get_size(&self) -> usize {
        self.get_size()
    }
    
    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error>{
        self.put(key, value)
    }
    
    fn get(&self, rank: usize, key: &str, len: usize) -> Result<Vec<u8>, Error>{
        self.get(rank, key, len)
    }

    fn exchange(&self) -> Result<(), Error>{
        self.exchange()
    }

    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Result<Vec<Vec<u8>>, Error>{
        
        self.exchange_data(key, value)
    }

    // fn barrier(&self) -> Result<(), Error> {
    //     self.barrier()
    // }
}

impl Error {
    pub(crate) fn from_pmi1_err_code(c_err: i32) -> Self {

        let kind = if c_err == pmi_sys::PMI_FAIL {
            ErrorKind::OperationFailed
        }
        else{
            let c_err = c_err as u32;
            match c_err {
                pmi_sys::PMI_ERR_INIT => ErrorKind::NotInitialized,
                pmi_sys::PMI_ERR_NOMEM => ErrorKind::NoBufSpaceAvailable,
                pmi_sys::PMI_ERR_INVALID_ARG => ErrorKind::InvalidArg,
                pmi_sys::PMI_ERR_INVALID_KEY => ErrorKind::InvalidKey,
                pmi_sys::PMI_ERR_INVALID_KEY_LENGTH => ErrorKind::InvalidKeyLength,
                pmi_sys::PMI_ERR_INVALID_VAL => ErrorKind::InvalidVal,
                pmi_sys::PMI_ERR_INVALID_VAL_LENGTH => ErrorKind::InvalidValLength,
                pmi_sys::PMI_ERR_INVALID_LENGTH => ErrorKind::InvalidLength,
                pmi_sys::PMI_ERR_INVALID_NUM_ARGS => ErrorKind::InvalidNumArgs,
                pmi_sys::PMI_ERR_INVALID_ARGS => ErrorKind::InvalidArgs,
                pmi_sys::PMI_ERR_INVALID_NUM_PARSED => ErrorKind::InvalidNumParsed,
                pmi_sys::PMI_ERR_INVALID_KEYVALP => ErrorKind::InvalidKeyValP,
                pmi_sys::PMI_ERR_INVALID_SIZE => ErrorKind::InvalidSize,
                pmi_sys::PMI_ERR_INVALID_KVS => ErrorKind::InvalidKVS,
                _ => ErrorKind::Other,
            }
        };

        Self {c_err, kind}
    }
}
