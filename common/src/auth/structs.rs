
#[derive(Default)]
struct CredentialBuilder {
    id: Option<String>,
    service: Service,
    username: String,
    secret: String,
    label: String,
}
