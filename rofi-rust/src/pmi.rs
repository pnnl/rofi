use std::{collections::HashMap, ffi::CString};

use debug_print::debug_println;

#[cfg(feature = "with-pmi2")]
extern crate pmi2_sys;

#[cfg(feature = "with-pmi2")]
pub struct Pmi2 {
    rank: usize,
    size: usize,
    finalize: bool,
    names: HashMap<String, usize>,
}


#[cfg(feature = "with-pmi2")]
impl Pmi2 {
    
    pub(crate) fn init() -> Self {
        
        let mut rank = 0;
        let mut size = 0;
        let mut spawned = 0;
        let mut appnum = 0;
        let mut finalize = false;

        if  unsafe{ pmi2_sys::PMI2_Initialized() } == 0 {
            let err = unsafe { pmi2_sys::PMI2_Init(&mut spawned as *mut i32, &mut size as *mut i32, &mut rank as *mut i32, &mut appnum as *mut i32)} as u32;
            if pmi2_sys::PMI2_SUCCESS != err {
                panic!("Could not init PMI2 {}", err);
            }
            finalize = true;
        }
        else {
            todo!()
        }

        debug_println!("Found rank: {}, size: {}, spawned: {}, appnum: {}", rank, size, spawned, appnum);
        
        Pmi2 {
            rank: rank as usize,
            size: size as usize,
            finalize,
            names: HashMap::new(),
        }
    }

    pub(crate) fn get_rank(&self) -> usize {
        
        self.rank
    }


    pub(crate) fn get_size(&self) -> usize {

        self.size
    }

    pub(crate) fn put(&self, key: &str, value: &[u8]) {
        let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",self.rank,key)).unwrap().into_raw();
        let kvs_val = self.encode(value);

        let err = unsafe { pmi2_sys::PMI2_KVS_Put(kvs_key, kvs_val.as_ptr() as *const i8) } as u32;
        if pmi2_sys::PMI2_SUCCESS != err { 
            panic!("Unable to set value in KVS {}", err);
        }
    }

    pub(crate) fn get(&self, rank: usize, key: &str, len: usize) -> Vec<u8>{
        let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",rank,key)).unwrap().into_raw();
        let mut kvs_val: Vec<u8> = vec![0; 2 * len + 1];
        let mut len = 0;
        if pmi2_sys::PMI2_SUCCESS != unsafe { pmi2_sys::PMI2_KVS_Get(std::ptr::null(), pmi2_sys::PMI2_ID_NULL ,kvs_key, kvs_val.as_mut_ptr() as *mut i8, kvs_val.len() as i32, &mut len as *mut i32) } as u32 {
            panic!("Unable to get value from KVS");
        }

        self.decode(&kvs_val)
    }

    pub(crate) fn exchange_data(&mut self, key: &str, value: &[u8]) -> Vec<Vec<u8>>{
        let res = self.names.get_mut(key);
        let new_key = key.to_owned() + & match res {
            Some(val) => {*val+=1; val.clone()},
            None => {self.names.insert(key.to_owned(), 0); 0}
        }.to_string();

        self.put(&new_key, value);

        self.exchange();

        let mut res = Vec::new();
        for i in 0..self.size as usize {
            let ret = self.get(i, &new_key, value.len());
            res.push(ret);
        }

        res
    }

    pub(crate) fn exchange(&self) {
        unsafe{ pmi2_sys::PMI2_KVS_Fence() };
    }
    
    /// PMI2 has no barrier call, so we use KVS_Fence for global synchronization
    /// Since implementations might do it lazily we also put and get a small buffer to 
    /// force the synchronization to happen.
    pub(crate) fn barrier(&self) {
        let data = 0;
        self.put("barrier", std::slice::from_ref(&data));
        
        self.exchange();

        let data_back = self.get((self.rank + 1) % self.size, "barrier", 1);
    }
}

#[cfg(feature = "with-pmi2")]
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

#[cfg(feature = "with-pmi1")]
extern crate pmi_sys;

#[cfg(feature = "with-pmi1")]
pub struct Pmi {
    rank: usize,
    size: usize,
    finalize: bool,
    kvs_name: std::ffi::CString,
    names: HashMap<String, usize>,
}

#[cfg(feature = "with-pmi1")]
impl Pmi {
    
    pub(crate) fn init() -> Self {
        
        let mut rank = 0;
        let mut size = 0;
        let mut spawned = 0;
        let mut appnum = 0;
        let mut finalize = false;
        let mut init= 0;
        let mut max_name_len: i32 = 0;

        if  unsafe{ pmi_sys::PMI_Initialized(&mut init as *mut i32) } as u32 == pmi_sys::PMI_SUCCESS {
            if init as u32 == pmi_sys::PMI_FALSE {
                let err = unsafe { pmi_sys::PMI_Init(&mut spawned as *mut i32)} as u32;
                if err != pmi_sys::PMI_SUCCESS {
                    panic!("Could not initialize PMI");
                }
                finalize = true;
            }
        }
        else {
            panic!("Could not check if PMI is initialized");
        }

        if unsafe{ pmi_sys::PMI_Get_size(&mut size as *mut i32)} as u32 != pmi_sys::PMI_SUCCESS {
            panic!("Could not retrieve job size from PMI");
        }

        if unsafe{ pmi_sys::PMI_Get_rank(&mut rank as *mut i32)} as u32 != pmi_sys::PMI_SUCCESS {
            panic!("Could not retrieve rank from PMI");
        }

        if unsafe{ pmi_sys::PMI_Get_appnum(&mut appnum as *mut i32)} as u32 != pmi_sys::PMI_SUCCESS {
            panic!("Could not retrieve app number from PMI");
        }



        if unsafe {pmi_sys::PMI_KVS_Get_name_length_max(&mut max_name_len as *mut i32)} as u32 != pmi_sys::PMI_SUCCESS {
            panic!("Could not retrieve name max length");
        }
        
        let mut name: Vec<u8> = vec![0; max_name_len as usize];
        if unsafe {pmi_sys::PMI_KVS_Get_my_name(name.as_mut_ptr() as *mut i8, max_name_len) } as u32 != pmi_sys::PMI_SUCCESS {
            panic!("Could not retrieve name");
        }
        let kvs_name = std::ffi::CString::new(name);
        
        let real_kvs_name = match kvs_name {
            Err(error) => {let len = error.nul_position(); std::ffi::CString::new(&error.into_vec()[..len]).unwrap() },
            Ok(real_name) => real_name,
        };

        debug_println!("Found rank: {}, size: {}, spawned: {}, appnum: {}", rank, size, spawned, appnum);
        
        Pmi {
            rank: rank as usize,
            size: size as usize,
            finalize,
            kvs_name: real_kvs_name,
            names: HashMap::new(),
        }
    }

    pub(crate) fn get_rank(&self) -> usize {
        
        self.rank
    }


    pub(crate) fn get_size(&self) -> usize {

        self.size
    }


    pub(crate) fn put(&self, key: &str, value: &[u8]) {
        let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",self.rank,key)).unwrap().into_raw();
        let kvs_val = self.encode(value);
        
        let err = unsafe { pmi_sys::PMI_KVS_Put(self.kvs_name.as_ptr(), kvs_key, kvs_val.as_ptr() as *const i8) } as u32;
        if pmi_sys::PMI_SUCCESS != err { 
            panic!("Unable to set value in KVS {}", err);
        }
        
        let kvs_key_ = unsafe { CString::from_raw(kvs_key) };
        debug_println!("P[{}] Putting to KVS {:?}\n\tKey: {:?}\n\tValue: {:x?}", self.get_rank(), self.kvs_name, kvs_key_, value);

    }
    
    pub(crate) fn get(&self, rank: usize, key: &str, len: usize) -> Vec<u8>{
        let kvs_key = std::ffi::CString::new(format!("rrofi-{}-{}",rank,key)).unwrap().into_raw();
        let mut kvs_val: Vec<u8> = vec![0; 2 * len + 1];
        
        let err = unsafe { pmi_sys::PMI_KVS_Get(self.kvs_name.as_ptr() ,kvs_key, kvs_val.as_mut_ptr() as *mut i8, kvs_val.len() as i32) } as u32 ;
        if pmi_sys::PMI_SUCCESS != err {
            panic!("Unable to get value from KVS {}", err);
        }
        
        let dec = self.decode(&kvs_val);
        let kvs_key_ = unsafe { CString::from_raw(kvs_key) };
        debug_println!("P[{}] Getting from KVS {:?}:\n\tKey: {:?}\n\tValue: {:x?}",  self.get_rank(), self.kvs_name, kvs_key_, dec);
        
        dec
    }

    pub(crate) fn exchange_data(&mut self, key: &str, value: &[u8]) -> Vec<Vec<u8>>{
        let res = self.names.get_mut(key);
        let new_key = key.to_owned() + & match res {
            Some(val) => {*val+=1; val.clone()},
            None => {self.names.insert(key.to_owned(), 0); 0}
        }.to_string();


        self.put(&new_key, value);

        self.exchange();

        let mut res = Vec::new();
        for i in 0..self.size as usize {
            let ret = self.get(i, &new_key, value.len());
            res.push(ret);
        }

        res
    }

    pub(crate) fn exchange(&self) {

        let err = unsafe{ pmi_sys::PMI_KVS_Commit(self.kvs_name.as_ptr()) } as u32;
        
        if err != pmi_sys::PMI_SUCCESS {
            panic!("Could not commit to kvs {}", err);
        }
        
        if unsafe{ pmi_sys::PMI_Barrier() } as u32 != pmi_sys::PMI_SUCCESS {
            panic!("PMI_Barrier failed")
        }
    }

    pub(crate) fn barrier(&self) {
        let err = unsafe { pmi_sys::PMI_Barrier() } as u32 ;

        if err != pmi_sys::PMI_SUCCESS {
            panic!("Could not call barrier")
        }
    }

    
}

#[cfg(feature = "with-pmi1")]
impl Drop for Pmi {
    fn drop(&mut self) {
        if self.finalize {
            let err = unsafe{ pmi_sys::PMI_Finalize() } as u32;
            if err != pmi_sys::PMI_SUCCESS {
                panic!("Failed to finalize PMI {}", err);
            }
        }
    }
}


pub(crate) trait PmiTrait {
    fn get_rank(&self) -> usize;
    fn get_size(&self) -> usize;
    fn put(&self, key: &str, value: &[u8]);
    fn get(&self, rank: usize, key: &str, len: usize) -> Vec<u8> ;
    fn exchange(&self);
    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Vec<Vec<u8>> ;
    fn barrier(&self);


    fn encode(&self, val: &[u8]) -> Vec<u8> {
        let mut res = vec![0; 2 * val.len() + 1];

        let encodings = vec!['0','1','2','3','4','5','6','7','8','9','a','b','c','d','e','f'];


        for i in 0..val.len() {
            res[2*i] = encodings[(val[i] & 0xf) as usize] as u8;
            res[2*i+1] = encodings[(val[i] >> 4) as usize] as u8;
        }

        res
    }

    fn decode(&self, val: &[u8]) -> Vec<u8> {
        let mut res = vec![0; val.len()/2];

        let mut j = 0;
        for i in 0..res.len() {
            if val[j] >= ('0' as u8) && val[j] <= ('9' as u8) {
                res[i] = val[j] - '0' as u8;
            }
            else {
                res[i] = val[j] - 'a' as u8 + 10;
            }
            j += 1;

            if val[j] >= '0' as u8 && val[j] <= '9' as u8 {
                res[i] |= (val[j] - '0' as u8 ) << 4;
            }
            else {
                res[i] |= ((val[j] - 'a' as u8) + 10) << 4;
            }

            j += 1;
        }
    
        res
    }
}

#[cfg(feature = "with-pmi2")]
impl PmiTrait for Pmi2 {
    fn get_rank(&self) -> usize {
        self.get_rank()
    }
    fn get_size(&self) -> usize {
        self.get_size()
    }
    
    fn put(&self, key: &str, value: &[u8]) {
        self.put(key, value)
    }
    
    fn get(&self, rank: usize, key: &str, len: usize) -> Vec<u8> {
        self.get(rank, key, len)
    }

    fn exchange(&self) {
        self.exchange();
    }

    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Vec<Vec<u8>>{
        
        self.exchange_data(key, value)
    }

    fn barrier(&self) {

        self.barrier();
    }
}

#[cfg(feature = "with-pmi1")]
impl PmiTrait for Pmi {
    fn get_rank(&self) -> usize {
        self.get_rank()
    }
    fn get_size(&self) -> usize {
        self.get_size()
    }
    
    fn put(&self, key: &str, value: &[u8]) {
        self.put(key, value)
    }
    
    fn get(&self, rank: usize, key: &str, len: usize) -> Vec<u8> {
        self.get(rank, key, len)
    }


    fn exchange(&self) {
        self.exchange();
    }

    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Vec<Vec<u8>>{
        
        self.exchange_data(key, value)
    }

    fn barrier(&self) {
        self.barrier()
    }
}
