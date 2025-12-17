#[derive(Debug)]
pub struct WatsonError {
    pub r#type: WatsonErrorType,
    pub error: String,
}
#[derive(Debug)]
pub enum WatsonErrorType {
    HttpGetRequest,
    Deserialization,
    UndefinedAttribute,

    FileOpen,
}
