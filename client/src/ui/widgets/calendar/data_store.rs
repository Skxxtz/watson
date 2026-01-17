use std::{
    cell::RefCell,
    collections::HashSet,
    fs,
    io::{BufReader, BufWriter},
    rc::Rc,
};

use chrono::Local;
use common::{
    auth::CredentialManager,
    calendar::utils::{CalDavEvent, CalEventType},
    utils::{
        errors::{WatsonError, WatsonErrorKind},
        paths::get_cache_dir,
    },
    watson_err,
};

use crate::{config::WidgetSpec, ui::widgets::calendar::types::CalendarRule};

#[derive(Debug, Default)]
pub struct CalendarDataStore {
    pub timed: Rc<RefCell<Vec<CalDavEvent>>>,
    pub allday: Rc<RefCell<Vec<CalDavEvent>>>,
    pub selection: Rc<RefCell<Option<CalendarRule>>>,
}
impl CalendarDataStore {
    pub fn new() -> Self {
        Self {
            timed: Rc::new(RefCell::new(Vec::new())),
            allday: Rc::new(RefCell::new(Vec::new())),
            selection: Rc::new(RefCell::new(None)),
        }
    }
    pub fn for_specs(&self, spec: &WidgetSpec) {
        if let WidgetSpec::Calendar { selection, .. } = spec {
            if selection.is_some() {
                *self.selection.borrow_mut() = selection.clone();
            }
        }
    }
    pub fn load_from_cache(&self) -> Result<(), WatsonError> {
        let mut path = get_cache_dir()?;
        path.push("calendar_cache.bin");

        if !path.exists() {
            return Ok(());
        }

        let file = fs::File::open(path)
            .map_err(|e| watson_err!(WatsonErrorKind::FileOpen, e.to_string()))?;

        let reader = BufReader::new(file);
        let (mut cached_timed, mut cached_allday): (Vec<CalDavEvent>, Vec<CalDavEvent>) =
            bincode::deserialize_from(reader)
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;

        // Cache invalidation
        let today = Local::now().date_naive();
        cached_timed.retain(|e| e.occurs_on_day(&today));
        cached_allday.retain(|e| e.occurs_on_day(&today));

        *self.timed.borrow_mut() = cached_timed;
        *self.allday.borrow_mut() = cached_allday;

        Ok(())
    }
    pub fn save_to_cache(&self) -> Result<(), WatsonError> {
        let mut path = get_cache_dir()?;
        path.push("calendar_cache.bin");

        let file = fs::File::create(path)
            .map_err(|e| watson_err!(WatsonErrorKind::FileOpen, e.to_string()))?;

        let writer = BufWriter::new(file);
        let data = (&*self.timed.borrow(), &*self.allday.borrow());
        bincode::serialize_into(writer, &data)
            .map_err(|e| watson_err!(WatsonErrorKind::Serialize, e.to_string()))?;

        Ok(())
    }
    pub async fn refresh(&self) -> usize {
        let mut credential_manager = match CredentialManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{:?}", e);
                return 0;
            }
        };
        if let Err(e) = credential_manager.unlock() {
            eprintln!("{:?}", e);
            return 0;
        }

        let today = Local::now().date_naive();
        let mut new_timed = Vec::new();
        let mut new_allday = Vec::new();
        let seen_ids: HashSet<String> = {
            let timed = self.timed.borrow();
            let allday = self.allday.borrow();

            let mut ids = HashSet::with_capacity(timed.len() + allday.len());

            ids.extend(timed.iter().map(|e| e.uid.clone()));
            ids.extend(allday.iter().map(|e| e.uid.clone()));
            ids
        };

        for account in credential_manager.credentials {
            let Some(mut provider) = account.provider() else {
                continue;
            };

            if let Err(e) = provider.init().await {
                // TODO: Log err
                eprintln!("{:?}", e);
                continue;
            }

            let calendars = match provider.get_calendars().await {
                Ok(v) => v,
                Err(e) => {
                    // TODO: Log err
                    eprintln!("{:?}", e);
                    continue;
                }
            };

            let mut events = match provider.get_events(calendars).await {
                Ok(v) => v,
                Err(e) => {
                    // TODO: Log err
                    eprintln!("{:?}", e);
                    continue;
                }
            };

            // Filter events
            if let Some(selection) = &*self.selection.borrow() {
                events.retain(|e| {
                    selection.is_allowed(&e.calendar_info.name) && e.occurs_on_day(&today)
                });
            } else {
                events.retain(|e| e.occurs_on_day(&today));
            }

            // Extend the Events
            {
                for item in events {
                    if !seen_ids.contains(&item.uid) {
                        item.seen.set(false);
                        match item.event_type {
                            CalEventType::Timed => new_timed.push(item),
                            CalEventType::AllDay => new_allday.push(item),
                        }
                    }
                }
            }
        }
        let num_changes = new_timed.len() + new_allday.len();
        if num_changes > 0 {
            {
                self.timed.borrow_mut().extend(new_timed);
                self.allday.borrow_mut().extend(new_allday);
            }

            let _ = self.save_to_cache();
        }
        num_changes
    }
}
