use crate::pmi;
use pmi::Error;

pub(crate) trait PmiTrait {
    fn get_rank(&self) -> usize;
    fn get_size(&self) -> usize;
    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), Error>;
    fn get(&self, rank: usize, key: &str, len: usize) -> Result<Vec<u8>, Error> ;
    fn exchange(&self) -> Result<(), Error>;
    fn exchange_data(&mut self, key: &str, value: &[u8]) -> Result<Vec<Vec<u8>>, Error> ;
    // fn barrier(&self) -> Result<(), Error>;


    fn encode(&self, val: &[u8]) -> Vec<u8> {
        let mut res = vec![0; 2 * val.len() + 1];

        let encodings = ['0','1','2','3','4','5','6','7','8','9','a','b','c','d','e','f'];


        for i in 0..val.len() {
            res[2*i] = encodings[(val[i] & 0xf) as usize] as u8;
            res[2*i+1] = encodings[(val[i] >> 4) as usize] as u8;
        }

        res
    }

    fn decode(&self, val: &[u8]) -> Vec<u8> {
        let mut res = vec![0; val.len()/2];

        let mut j = 0;
        for el in &mut res {
            if val[j] >= (b'0') && val[j] <= (b'9') {
                *el = val[j] - b'0';
            }
            else {
                *el = val[j] - b'a' + 10;
            }
            j += 1;

            if val[j] >= b'0' && val[j] <= b'9' {
                *el |= (val[j] - b'0' ) << 4;
            }
            else {
                *el |= ((val[j] - b'a') + 10) << 4;
            }

            j += 1;
        }
    
        res
    }
}