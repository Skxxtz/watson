use std::{
    fmt::Display,
    io::{Read, StdinLock, Write, stdin, stdout},
};

use serde::{Deserialize, Serialize};

use crate::{
    auth::{Credential, CredentialData, CredentialManager, credentials::CredentialSecret},
    errors::WatsonError,
};

// ---------- TUI ----------

pub struct AuthTui<'a> {
    stdin: StdinLock<'a>,
    manager: CredentialManager,
    _guard: RawModeGuard,
}

impl<'a> AuthTui<'a> {
    pub fn new() -> Result<Self, WatsonError> {
        let stdin = stdin();
        let stdin_lock = stdin.lock();

        let guard = RawModeGuard::new().expect("Failed to enable raw mode");
        let mut manager = CredentialManager::new()?;
        manager.unlock()?;

        Ok(Self {
            _guard: guard,
            stdin: stdin_lock,
            manager,
        })
    }

    pub fn run(&mut self) -> Result<(), WatsonError> {
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
                        update_new_account(s, read_input(&mut self.stdin), &mut self.manager)?
                    {
                        state = next;
                    }
                }
                UiState::ManageAccounts(s) => {
                    render_manage(s, &self.manager);
                    if let Some(next) =
                        update_manage(s, read_input(&mut self.stdin), &mut self.manager)
                    {
                        state = next;
                    }
                }
                UiState::ManageOptions(s) => {
                    render_manage_options_menu(s.selected);
                    if let Some(next) = update_manage_options_menu(
                        s,
                        read_input(&mut self.stdin),
                        &mut self.manager,
                    )? {
                        state = next;
                    }
                }
                UiState::Edit(s) => {
                    render_edit(s, &mut self.manager);
                    if let Some(next) =
                        update_edit(s, read_input(&mut self.stdin), &mut self.manager)?
                    {
                        state = next;
                    }
                }
                UiState::ServiceEdit(s) => {
                    render_service_selection(s);
                    if let Some(next) =
                        update_service_selection(s, read_input(&mut self.stdin), &mut self.manager)
                    {
                        state = next;
                    }
                }
                UiState::Quit => break,
            }
        }

        Ok(())
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
    OpenBrowser,
    Save,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum CredentialService {
    Icloud,
    Google,
    None,
}
impl CredentialService {
    const LEN: usize = 2;
    const ALL: [Self; 2] = [Self::Icloud, Self::Google];
    fn itos(index: usize) -> Self {
        match index {
            0 => Self::Icloud,
            1 => Self::Google,
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
            Self::Google => "Google",
            Self::None => "",
        };
        write!(f, "{}", st)
    }
}

struct NewAccountState {
    field: AccountField,
    service: CredentialService,
    data: CredentialData,
    label: CredentialSecret,
}
struct EditState {
    field: AccountField,
    cred: usize,
    initial_secret: CredentialSecret,
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
            data: CredentialData::Empty,
            label: CredentialSecret::Decrypted(String::new()),
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

    match &s.data {
        CredentialData::Password { username, secret } => {
            println!(
                "{} Username: {}",
                if matches!(s.field, AccountField::Username) {
                    ">"
                } else {
                    " "
                },
                username
            );

            println!(
                "{} Password: {}",
                if matches!(s.field, AccountField::Password) {
                    ">"
                } else {
                    " "
                },
                "*".repeat(secret.len())
            );
        }
        CredentialData::OAuth { .. } => {
            println!(
                "{} Proceed in Broser →",
                if matches!(s.field, AccountField::OpenBrowser) {
                    ">"
                } else {
                    " "
                }
            );
        }
        CredentialData::Empty => {}
    }

    if !matches!(s.service, CredentialService::None) {
        println!(
            "{} Label: {}",
            if matches!(s.field, AccountField::Label) {
                ">"
            } else {
                " "
            },
            s.label
        );
    }

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
    manager: &mut CredentialManager,
) -> Result<Option<UiState>, WatsonError> {
    let current = match &mut s.data {
        CredentialData::Password { username, secret } => match s.field {
            AccountField::Username => Some(username),
            AccountField::Password => Some(secret),
            AccountField::Label => Some(&mut s.label),
            _ => None,
        },
        CredentialData::OAuth { .. } => match s.field {
            AccountField::Label => Some(&mut s.label),
            _ => None,
        },
        CredentialData::Empty => None,
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
            s.field = match s.service {
                CredentialService::Icloud => AccountField::Username,
                CredentialService::Google => AccountField::OpenBrowser,
                _ => AccountField::Service,
            };
        }
        Input::Up => {
            s.field = match s.field {
                AccountField::Service => AccountField::Service,
                AccountField::Username => AccountField::Service,
                AccountField::Password => AccountField::Username,
                AccountField::Label => match s.service {
                    CredentialService::Google => AccountField::OpenBrowser,
                    CredentialService::Icloud => AccountField::Password,
                    _ => AccountField::Service,
                },
                _ => AccountField::Service,
            };
        }
        Input::Enter | Input::Tab | Input::Down
            if !current.as_ref().map_or(false, |f| f.is_empty()) =>
        {
            s.field = match s.field {
                AccountField::Service => {
                    return Ok(Some(UiState::ServiceEdit(ServiceSelectState {
                        return_target: ServiceReturnTarget::NewAccount,
                        cred_index: None,
                        selected: 0,
                    })));
                }
                AccountField::Username => AccountField::Password,
                AccountField::Password => AccountField::Label,
                AccountField::OpenBrowser => AccountField::Label,
                AccountField::Label => {
                    // Save to credential manager
                    let cred = Credential::new(s.data.clone(), s.service, s.label.take());
                    manager.insert(cred);
                    manager.save()?;

                    return Ok(Some(UiState::MainMenu));
                }
                _ => AccountField::Service,
            };
        }
        Input::Esc => return Ok(Some(UiState::MainMenu)),
        _ => {}
    }
    Ok(None)
}

// ---------- Service Selection ------------
fn update_service_selection(
    state: &mut ServiceSelectState,
    input: Input,
    manager: &mut CredentialManager,
) -> Option<UiState> {
    match input {
        Input::Up if state.selected > 0 => state.selected -= 1,
        Input::Down | Input::Tab if state.selected < CredentialService::LEN - 1 => {
            state.selected += 1
        }
        Input::Enter => {
            return Some(match state.return_target {
                ServiceReturnTarget::EditAccount => {
                    let current = &mut manager.credentials[state.cred_index.unwrap()];
                    match &mut current.data {
                        CredentialData::Password { username, secret } => {
                            println!("{} - {} - {}", secret, username, current.label);
                            current.service = CredentialService::itos(state.selected);
                            UiState::Edit(EditState {
                                field: AccountField::Service,
                                cred: state.cred_index.unwrap(),
                                initial_secret: std::mem::take(secret),
                            })
                        }
                        _ => UiState::ManageAccounts(ManageState {
                            selected: state.cred_index.unwrap_or(0),
                        }),
                    }
                }
                ServiceReturnTarget::NewAccount => {
                    let service = CredentialService::itos(state.selected);
                    let data = match &service {
                        CredentialService::Google => CredentialData::OAuth {
                            client_id: CredentialSecret::Decrypted(String::new()),
                            client_secret: CredentialSecret::Decrypted(String::new()),
                            access_token: CredentialSecret::Decrypted(String::new()),
                            refresh_token: CredentialSecret::Decrypted(String::new()),
                            expires_at: 0,
                        },
                        CredentialService::Icloud => CredentialData::Password {
                            username: CredentialSecret::Decrypted(String::new()),
                            secret: CredentialSecret::Decrypted(String::new()),
                        },
                        CredentialService::None => CredentialData::Empty,
                    };

                    UiState::NewAccount(NewAccountState {
                        field: AccountField::Service,
                        service,
                        data,
                        label: CredentialSecret::Decrypted(String::new()),
                    })
                }
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

fn render_manage(s: &ManageState, creds: &CredentialManager) {
    clear();
    println!("Accounts:\n");

    for (i, c) in creds.credentials.iter().enumerate() {
        match &c.data {
            CredentialData::Password { username, .. } => {
                if let CredentialSecret::Decrypted(username) = username {
                    if i == s.selected {
                        println!("> {} ({})", c.label, username);
                    } else {
                        println!("  {} ({})", c.label, username);
                    }
                }
            }
            CredentialData::OAuth { .. } => {
                if i == s.selected {
                    println!("> {}", c.label);
                } else {
                    println!("  {}", c.label);
                }
            }
            CredentialData::Empty => {
                println!("Debug: Empty account")
            }
        }
    }

    println!("\nEsc: back");
}

fn update_manage(
    s: &mut ManageState,
    input: Input,
    manager: &CredentialManager,
) -> Option<UiState> {
    match input {
        Input::Up if s.selected > 0 => s.selected -= 1,
        Input::Down | Input::Tab if s.selected < manager.credentials.len() - 1 => s.selected += 1,
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
    manager: &mut CredentialManager,
) -> Result<Option<UiState>, WatsonError> {
    match input {
        Input::Up if state.selected > 0 => state.selected -= 1,
        Input::Down | Input::Tab if state.selected < ManageOptionsState::OPTIONS.len() - 1 => {
            state.selected += 1
        }
        Input::Enter => {
            return Ok(Some(match state.selected {
                0 => {
                    let current = &mut manager.credentials[state.cred_index];
                    match &mut current.data {
                        CredentialData::Password { secret, .. } => {
                            let edit_state = EditState {
                                field: AccountField::Service,
                                cred: state.cred_index,
                                initial_secret: std::mem::take(secret),
                            };
                            UiState::Edit(edit_state)
                        }
                        _ => return Ok(None),
                    }
                }
                1 => {
                    // Remove entry from credential manager
                    if manager.delete_index(state.selected).is_some() {
                        manager.save()?;
                    }

                    UiState::ManageAccounts(ManageState {
                        selected: state.cred_index.saturating_sub(1),
                    })
                }
                _ => UiState::Quit,
            }));
        }
        _ => {}
    }
    Ok(None)
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
fn render_edit(s: &EditState, manager: &mut CredentialManager) {
    let cred = &mut manager.credentials[s.cred];
    let CredentialData::Password { username, secret } = &cred.data else {
        return;
    };
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
        username
    );

    if let CredentialSecret::Decrypted(secret) = secret {
        println!(
            "{} Password: {}",
            if matches!(s.field, AccountField::Password) {
                ">"
            } else {
                " "
            },
            "*".repeat(secret.len())
        );
    }

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
    manager: &mut CredentialManager,
) -> Result<Option<UiState>, WatsonError> {
    let cred = &mut manager.credentials[s.cred];
    let current = match &mut cred.data {
        CredentialData::Password { username, secret } => match s.field {
            AccountField::Username => Some(username),
            AccountField::Password => Some(secret),
            _ => None,
        },
        _ => None,
    };
    let current_text = if current.is_none() {
        match s.field {
            AccountField::Label => Some(&mut cred.label),
            _ => None,
        }
    } else {
        None
    };

    match input {
        Input::Char(c) => {
            if let Some(CredentialSecret::Decrypted(value)) = current {
                value.push(c);
            }
            current_text.map(|f| f.push(c));
        }
        Input::String(s) => {
            if let Some(CredentialSecret::Decrypted(value)) = current {
                value.push_str(&s);
            }
            current_text.map(|f| f.push_str(&s));
        }
        Input::Backspace => {
            if let Some(CredentialSecret::Decrypted(value)) = current {
                value.pop();
            }
            current_text.map(|f| f.pop());
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
                AccountField::OpenBrowser => AccountField::Service,
            };
        }
        Input::Enter | Input::Tab | Input::Down => {
            s.field = match s.field {
                AccountField::Service => {
                    match &mut cred.data {
                        CredentialData::Password { secret, .. } => {
                            *secret = std::mem::take(&mut s.initial_secret);
                        }
                        _ => {}
                    }
                    return Ok(Some(UiState::ServiceEdit(ServiceSelectState {
                        cred_index: Some(s.cred),
                        return_target: ServiceReturnTarget::EditAccount,
                        selected: 0,
                    })));
                }
                AccountField::Username => AccountField::Password,
                AccountField::Password => AccountField::Label,
                AccountField::Label => AccountField::Save,
                AccountField::OpenBrowser => AccountField::OpenBrowser,
                AccountField::Save => {
                    // Important: return the initial secret if it wasnt changed
                    match &mut cred.data {
                        CredentialData::Password { secret, .. } => {
                            if secret.is_empty() {
                                *secret = std::mem::take(&mut s.initial_secret);
                            }
                        }
                        _ => {}
                    }

                    // Save
                    manager.save()?;

                    return Ok(Some(UiState::ManageAccounts(ManageState { selected: 0 })));
                }
            };
        }
        Input::Esc => {
            match &mut cred.data {
                CredentialData::Password { secret, .. } => {
                    *secret = std::mem::take(&mut s.initial_secret);
                }
                _ => {}
            }
            return Ok(Some(UiState::ManageAccounts(ManageState { selected: 0 })));
        }
    }
    Ok(None)
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
