pub struct Error {
    pub(crate) c_err: i32,
    pub kind : ErrorKind,
}

pub enum ErrorKind {
    NotInitialized,
    NoBufSpaceAvailable,
    InvalidArg,
    InvalidKey,
    InvalidKeyLength,
    InvalidVal,
    InvalidValLength,
    InvalidLength,
    InvalidNumArgs,
    InvalidArgs,
    InvalidNumParsed,
    InvalidKeyValP,
    InvalidSize,
    InvalidKVS,
    OperationFailed,
    Other,
}




impl std::fmt::Debug for Error {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

        write!(f, "Error {}", self.c_err)
    }
}