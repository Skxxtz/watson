#[macro_export]
macro_rules! watson_err {
    ($kind:expr, $msg:expr) => {
        WatsonError {
            kind: $kind,
            message: $msg.into(),
            file: file!(),
            line: line!(),
        }
    };
}

#[derive(Debug)]
pub struct WatsonError {
    pub kind: WatsonErrorKind,
    pub message: String,
    pub file: &'static str,
    pub line: u32,
}

#[derive(Debug)]
pub enum WatsonErrorKind {
    HttpGetRequest,
    Deserialization,
    Serialization,

    UndefinedAttribute,
    InvalidAttribute,

    FileOpen,
    FileCreate,
    FileRead,
    FileWrite,
    FileExist,
    FileMetadata,

    DirExist,
    DirCreate,

    Base64Encode,
    Base64Decode,

    Decryption,
    Encryption,

    EnvVar,

    CredentialEntry,
    CredentialRead,
}
