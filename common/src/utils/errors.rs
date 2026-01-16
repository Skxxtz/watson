#[macro_export]
macro_rules! watson_err {
    // Case with just a message literal
    ($kind:expr, $msg:expr) => {
        WatsonError {
            kind: $kind,
            message: $msg.into(),
            file: file!(),
            line: line!(),
        }
    };
    // Case with message + format arguments
    ($kind:expr, $fmt:expr, $($args:tt)*) => {
        WatsonError {
            kind: $kind,
            message: format!($fmt, $($args)*),
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
    GoogleAuth,
    GoogleCalendar,

    CommandExecute,

    HttpPostRequest,
    HttpGetRequest,
    Deserialize,
    Serialize,

    DateParse,

    UndefinedAttribute,
    InvalidAttribute,
    InvalidData,

    IO,
    TaskJoin,

    FileOpen,
    FileCreate,
    FileRead,
    FileWrite,
    FileExist,
    FileMetadata,

    DirExist,
    DirCreate,
    DirRead,

    Base64Encode,
    Base64Decode,

    TcpServer,
    StreamRead,
    StreamWrite,
    StreamBind,
    StreamConnect,
    StreamListener,

    UrlFormat,

    Decryption,
    Encryption,

    EnvVar,

    CredentialEntry,
    CredentialRead,

    ProxyCreate,
    DBusConnect,
    DBusPropertySet,
    DBusPropertyGet,
    DBusProxyCall,

    BluetoothServiceDisabled,
    BacklightNotFound,

    Audio,
    Todo,
}
