use std::{
    fmt::Display,
    io::{Read, StdinLock, Write, stdin, stdout},
};

use crate::auth::{Credential, CredentialManager, credentials};

#[derive(Default, Debug, Clone)]
pub struct CredentialBuilder {
    pub id: Option<String>,
    pub service: CredentialService,
    pub username: String,
    pub secret: String,
    pub label: String,
}
impl CredentialBuilder {
    pub fn from_credential(cred: Credential, service: CredentialService) -> Self {
        Self {
            id: Some(cred.id),
            service,
            username: cred.username,
            secret: cred.secret,
            label: cred.label,
        }
    }
}
// ---------- TUI ----------

pub struct AuthTui<'a> {
    stdin: StdinLock<'a>,
    credentials: Vec<CredentialBuilder>,
    _guard: RawModeGuard,
}

impl<'a> AuthTui<'a> {
    pub fn new() -> Self {
        let stdin = stdin();
        let stdin_lock = stdin.lock();

        let guard = RawModeGuard::new().expect("Failed to enable raw mode");
        let credentials: Vec<CredentialBuilder> = CredentialService::ALL
            .into_iter()
            .filter_map(|s| {
                CredentialManager::new(&s.name())
                    .and_then(|m| m.get_credential_builders(s))
                    .ok()
            })
            .flatten()
            .collect();

        Self {
            _guard: guard,
            stdin: stdin_lock,
            credentials,
        }
    }

    pub fn run(&mut self) {
        let mut state = UiState::MainMenu;
        let mut menu = MenuState { selected: 0 };

        loop {
            match &mut state {
                UiState::MainMenu => {
                    render_main_menu(menu.selected);
                    if let Some(next) = update_main_menu(&mut menu, read_input(&mut self.stdin)) {
                        state = next;
                    }
                }
                UiState::NewAccount(s) => {
                    render_new_account(s);
                    if let Some(next) =
                        update_new_account(s, read_input(&mut self.stdin), &mut self.credentials)
                    {
                        state = next;
                    }
                }
                UiState::ManageAccounts(s) => {
                    render_manage(s, &self.credentials);
                    if let Some(next) =
                        update_manage(s, read_input(&mut self.stdin), &mut self.credentials)
                    {
                        state = next;
                    }
                }
                UiState::ManageOptions(s) => {
                    render_manage_options_menu(s.selected);
                    if let Some(next) = update_manage_options_menu(
                        s,
                        read_input(&mut self.stdin),
                        &mut self.credentials,
                    ) {
                        state = next;
                    }
                }
                UiState::Edit(s) => {
                    render_edit(s, &mut self.credentials);
                    if let Some(next) =
                        update_edit(s, read_input(&mut self.stdin), &mut self.credentials)
                    {
                        state = next;
                    }
                }
                UiState::ServiceEdit(s) => {
                    render_service_selection(s);
                    if let Some(next) = update_service_selection(
                        s,
                        read_input(&mut self.stdin),
                        &mut self.credentials,
                    ) {
                        state = next;
                    }
                }
                UiState::Quit => break,
            }
        }
    }
}

// ---------- State ----------

const MAIN_OPTIONS: [&str; 3] = [
    "Configure new account",
    "Manage existing credentials",
    "Quit",
];

enum UiState {
    MainMenu,
    NewAccount(NewAccountState),
    ManageAccounts(ManageState),
    ManageOptions(ManageOptionsState),
    Edit(EditState),
    ServiceEdit(ServiceSelectState),
    Quit,
}

struct MenuState {
    selected: usize,
}

struct ManageState {
    selected: usize,
}

struct ManageOptionsState {
    selected: usize,
    cred_index: usize,
}
impl ManageOptionsState {
    const OPTIONS: [&str; 2] = ["Edit", "Delete"];
}

enum Input {
    Up,
    Down,
    Tab,
    Enter,
    Esc,
    Char(char),
    String(String),
    Backspace,
}

enum AccountField {
    Service,
    Username,
    Password,
    Label,
    Save,
}

#[derive(Debug, Clone, Copy)]
pub enum CredentialService {
    Icloud,
    None,
}
impl CredentialService {
    const LEN: usize = 2;
    const ALL: [Self; 1] = [Self::Icloud];
    fn name(&self) -> String {
        let st = match self {
            Self::Icloud => "icloud",
            Self::None => "",
        };
        st.to_string()
    }
    fn itos(index: usize) -> Self {
        match index {
            0 => Self::Icloud,
            _ => Self::None,
        }
    }
    fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }
}
impl Default for CredentialService {
    fn default() -> Self {
        Self::None
    }
}
impl Display for CredentialService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let st = match self {
            Self::Icloud => "ICloud",
            Self::None => "",
        };
        write!(f, "{}", st)
    }
}

struct NewAccountState {
    field: AccountField,
    service: CredentialService,
    username: String,
    secret: String,
    label: String,
}
struct EditState {
    field: AccountField,
    cred: usize,
    initial_secret: String,
}

struct ServiceSelectState {
    return_target: ServiceReturnTarget,
    cred_index: Option<usize>,
    selected: usize,
}

enum ServiceReturnTarget {
    NewAccount,
    EditAccount,
}

impl NewAccountState {
    fn new() -> Self {
        Self {
            service: CredentialService::None,
            field: AccountField::Service,
            username: String::new(),
            secret: String::new(),
            label: String::new(),
        }
    }
}

// ---------- Input handling ----------

fn read_input(handle: &mut StdinLock) -> Input {
    let mut buf = [0u8; 32]; // max sequence length
    let n = handle.read(&mut buf).unwrap(); // blocking read

    match &buf[..n] {
        [0x1b, 0x5b, 0x41] => Input::Up,
        [0x1b, 0x5b, 0x42] => Input::Down,
        [0x1b] => Input::Esc,
        [b'\t'] => Input::Tab,
        [b'\r'] | [b'\n'] => Input::Enter,
        [0x7f] => Input::Backspace,
        [c] => Input::Char(*c as char),
        _ => Input::String(String::from_utf8_lossy(&buf[..n]).to_string()),
    }
}

// ---------- Main menu ----------

fn update_main_menu(state: &mut MenuState, input: Input) -> Option<UiState> {
    match input {
        Input::Up if state.selected > 0 => state.selected -= 1,
        Input::Down | Input::Tab if state.selected < MAIN_OPTIONS.len() - 1 => state.selected += 1,
        Input::Enter => {
            return Some(match state.selected {
                0 => UiState::NewAccount(NewAccountState::new()),
                1 => UiState::ManageAccounts(ManageState { selected: 0 }),
                _ => UiState::Quit,
            });
        }
        _ => {}
    }
    None
}

fn render_main_menu(selected: usize) {
    clear();
    println!("Select an option:\n");
    for (i, label) in MAIN_OPTIONS.iter().enumerate() {
        if i == selected {
            println!("> {}", label);
        } else {
            println!("  {}", label);
        }
    }
}

// ---------- New account ----------

fn render_new_account(s: &NewAccountState) {
    clear();
    println!("Create new account:\n");

    println!(
        "{} Service: {}",
        if matches!(s.field, AccountField::Service) {
            ">"
        } else {
            " "
        },
        s.service
    );

    println!(
        "{} Username: {}",
        if matches!(s.field, AccountField::Username) {
            ">"
        } else {
            " "
        },
        s.username
    );

    println!(
        "{} Password: {}",
        if matches!(s.field, AccountField::Password) {
            ">"
        } else {
            " "
        },
        "*".repeat(s.secret.len())
    );

    println!(
        "{} Label: {}",
        if matches!(s.field, AccountField::Label) {
            ">"
        } else {
            " "
        },
        s.label
    );

    match s.field {
        AccountField::Service if s.service.is_empty() => {
            println!("\nEnter: choose service • Esc: cancel");
        }
        _ => {
            println!("\nType to edit • ↑↓ navigate • Esc: cancel");
        }
    }
}

fn update_new_account(
    s: &mut NewAccountState,
    input: Input,
    credntials: &mut Vec<CredentialBuilder>,
) -> Option<UiState> {
    let current = match s.field {
        AccountField::Username => Some(&mut s.username),
        AccountField::Password => Some(&mut s.secret),
        AccountField::Label => Some(&mut s.label),
        _ => None,
    };

    match input {
        Input::Char(c) => {
            current.map(|f| f.push(c));
        }
        Input::String(s) => {
            current.map(|f| f.push_str(&s));
        }
        Input::Backspace => {
            current.map(|f| f.pop());
        }
        Input::Down | Input::Tab if matches!(s.field, AccountField::Service) => {
            if !matches!(s.service, CredentialService::None) {
                s.field = AccountField::Username;
            }
        }
        Input::Up => {
            s.field = match s.field {
                AccountField::Service => AccountField::Service,
                AccountField::Username => AccountField::Service,
                AccountField::Password => AccountField::Username,
                AccountField::Label => AccountField::Password,
                _ => AccountField::Service,
            };
        }
        Input::Enter | Input::Tab | Input::Down
            if !current.as_ref().map_or(false, |f| f.is_empty()) =>
        {
            s.field = match s.field {
                AccountField::Service => {
                    return Some(UiState::ServiceEdit(ServiceSelectState {
                        return_target: ServiceReturnTarget::NewAccount,
                        cred_index: None,
                        selected: 0,
                    }));
                }
                AccountField::Username => AccountField::Password,
                AccountField::Password => AccountField::Label,
                AccountField::Label => {
                    // Save to credential manager
                    if let Ok(mut manager) = CredentialManager::new(&s.service.name()) {
                        if let Ok(cred) =
                            manager.store(s.username.clone(), s.secret.clone(), s.label.clone())
                        {
                            credntials.push(CredentialBuilder::from_credential(cred, s.service));
                        }
                    }

                    return Some(UiState::MainMenu);
                }
                _ => AccountField::Service,
            };
        }
        Input::Esc => return Some(UiState::MainMenu),
        _ => {}
    }
    None
}

// ---------- Service Selection ------------
fn update_service_selection(
    state: &mut ServiceSelectState,
    input: Input,
    credentials: &mut [CredentialBuilder],
) -> Option<UiState> {
    match input {
        Input::Up if state.selected > 0 => state.selected -= 1,
        Input::Down | Input::Tab if state.selected < CredentialService::LEN - 1 => {
            state.selected += 1
        }
        Input::Enter => {
            return Some(match state.return_target {
                ServiceReturnTarget::EditAccount => {
                    let current = &mut credentials[state.cred_index.unwrap()];
                    println!(
                        "{} - {} - {}",
                        current.secret, current.username, current.label
                    );
                    current.service = CredentialService::itos(state.selected);
                    UiState::Edit(EditState {
                        field: AccountField::Service,
                        cred: state.cred_index.unwrap(),
                        initial_secret: std::mem::take(&mut current.secret),
                    })
                }
                ServiceReturnTarget::NewAccount => UiState::NewAccount(NewAccountState {
                    field: AccountField::Service,
                    service: CredentialService::itos(state.selected),
                    username: String::new(),
                    secret: String::new(),
                    label: String::new(),
                }),
            });
        }
        _ => {}
    }
    None
}

fn render_service_selection(state: &mut ServiceSelectState) {
    clear();
    println!("Select a service:\n");
    for (i, label) in CredentialService::ALL.iter().enumerate() {
        if i == state.selected {
            println!("> {}", label);
        } else {
            println!("  {}", label);
        }
    }
}
// ---------- Manage accounts ----------

fn render_manage(s: &ManageState, creds: &[CredentialBuilder]) {
    clear();
    println!("Accounts:\n");

    for (i, c) in creds.iter().enumerate() {
        if i == s.selected {
            println!("> {} ({})", c.label, c.username);
        } else {
            println!("  {} ({})", c.label, c.username);
        }
    }

    println!("\nEsc: back");
}

fn update_manage(
    s: &mut ManageState,
    input: Input,
    creds: &mut Vec<CredentialBuilder>,
) -> Option<UiState> {
    match input {
        Input::Up if s.selected > 0 => s.selected -= 1,
        Input::Down | Input::Tab if s.selected < creds.len() - 1 => s.selected += 1,
        Input::Esc => return Some(UiState::MainMenu),
        Input::Enter => {
            return Some(UiState::ManageOptions(ManageOptionsState {
                selected: 0,
                cred_index: s.selected,
            }));
        }
        _ => {}
    }
    None
}
// ----------- Manage Options -----------
fn update_manage_options_menu(
    state: &mut ManageOptionsState,
    input: Input,
    credentials: &mut Vec<CredentialBuilder>,
) -> Option<UiState> {
    match input {
        Input::Up if state.selected > 0 => state.selected -= 1,
        Input::Down | Input::Tab if state.selected < ManageOptionsState::OPTIONS.len() - 1 => {
            state.selected += 1
        }
        Input::Enter => {
            let current = &mut credentials[state.cred_index];
            return Some(match state.selected {
                0 => {
                    let edit_state = EditState {
                        field: AccountField::Service,
                        cred: state.cred_index,
                        initial_secret: std::mem::take(&mut current.secret),
                    };
                    UiState::Edit(edit_state)
                }
                1 => {
                    // Remove entry from credential manager
                    if let Some(id) = &current.id {
                        CredentialManager::new(&current.service.name())
                            .ok()
                            .and_then(|mut manager| manager.remove_credential(id).ok())
                            .map(|_| {
                                credentials.remove(state.cred_index);
                            });
                    }

                    UiState::ManageAccounts(ManageState {
                        selected: state.cred_index.saturating_sub(1),
                    })
                }
                _ => UiState::Quit,
            });
        }
        _ => {}
    }
    None
}

fn render_manage_options_menu(selected: usize) {
    clear();
    println!("Select an option:\n");
    for (i, label) in ManageOptionsState::OPTIONS.iter().enumerate() {
        if i == selected {
            println!("> {}", label);
        } else {
            println!("  {}", label);
        }
    }
}

// --------- Edit View ------------
//
fn render_edit(s: &EditState, credentials: &mut [CredentialBuilder]) {
    let cred = &mut credentials[s.cred];
    clear();
    println!("Edit Account:\n");

    println!(
        "{} Service: {}",
        if matches!(s.field, AccountField::Service) {
            ">"
        } else {
            " "
        },
        cred.service
    );

    println!(
        "{} Username: {}",
        if matches!(s.field, AccountField::Username) {
            ">"
        } else {
            " "
        },
        cred.username
    );

    println!(
        "{} Password: {}",
        if matches!(s.field, AccountField::Password) {
            ">"
        } else {
            " "
        },
        "*".repeat(cred.secret.len())
    );

    println!(
        "{} Label: {}",
        if matches!(s.field, AccountField::Label) {
            ">"
        } else {
            " "
        },
        cred.label
    );

    println!(
        "{} Save",
        if matches!(s.field, AccountField::Save) {
            ">"
        } else {
            " "
        },
    );

    match s.field {
        AccountField::Service if cred.service.is_empty() => {
            println!("\nEnter: change service • Esc: cancel");
        }
        _ => {
            println!("\nType to edit • ↑↓ navigate • Esc: cancel");
        }
    }
}

fn update_edit(
    s: &mut EditState,
    input: Input,
    credentials: &mut [CredentialBuilder],
) -> Option<UiState> {
    let cred = &mut credentials[s.cred];
    let current = match s.field {
        AccountField::Username => Some(&mut cred.username),
        AccountField::Password => Some(&mut cred.secret),
        AccountField::Label => Some(&mut cred.label),
        _ => None,
    };

    match input {
        Input::Char(c) => {
            current.map(|f| f.push(c));
        }
        Input::String(s) => {
            current.map(|f| f.push_str(&s));
        }
        Input::Backspace => {
            current.map(|f| f.pop());
        }
        Input::Down | Input::Tab if matches!(s.field, AccountField::Service) => {
            if !matches!(cred.service, CredentialService::None) {
                s.field = AccountField::Username;
            }
        }
        Input::Up => {
            s.field = match s.field {
                AccountField::Service => AccountField::Service,
                AccountField::Username => AccountField::Service,
                AccountField::Password => AccountField::Username,
                AccountField::Label => AccountField::Password,
                AccountField::Save => AccountField::Label,
            };
        }
        Input::Enter | Input::Tab | Input::Down => {
            s.field = match s.field {
                AccountField::Service => {
                    cred.secret = std::mem::take(&mut s.initial_secret);
                    return Some(UiState::ServiceEdit(ServiceSelectState {
                        cred_index: Some(s.cred),
                        return_target: ServiceReturnTarget::EditAccount,
                        selected: 0,
                    }));
                }
                AccountField::Username => AccountField::Password,
                AccountField::Password => AccountField::Label,
                AccountField::Label => AccountField::Save,
                AccountField::Save => {
                    if cred.secret.is_empty() {
                        cred.secret = std::mem::take(&mut s.initial_secret);
                    }

                    // Save
                    let _ = CredentialManager::new(&cred.service.name())
                        .and_then(|mut m| m.update_credential(cred.clone()));

                    return Some(UiState::ManageAccounts(ManageState { selected: 0 }));
                }
            };
        }
        Input::Esc => {
            cred.secret = std::mem::take(&mut s.initial_secret);
            return Some(UiState::ManageAccounts(ManageState { selected: 0 }));
        }
    }
    None
}

// ---------- Terminal utils ----------

fn clear() {
    print!("\x1b[2J\x1b[H");
    stdout().flush().unwrap();
}

// ---------- Raw mode ----------

#[cfg(unix)]
fn enable_raw_mode() -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    use termios::*;
    let fd = std::io::stdin().as_raw_fd();
    let mut term = Termios::from_fd(fd)?;
    term.c_lflag &= !(ICANON | ECHO);
    tcsetattr(fd, TCSANOW, &term)?;
    Ok(())
}

#[cfg(unix)]
fn disable_raw_mode() -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    use termios::*;
    let fd = std::io::stdin().as_raw_fd();
    let mut term = Termios::from_fd(fd)?;
    term.c_lflag |= ICANON | ECHO;
    tcsetattr(fd, TCSANOW, &term)?;
    Ok(())
}

struct RawModeGuard;
impl RawModeGuard {
    fn new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}
