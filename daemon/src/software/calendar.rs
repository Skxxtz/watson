use std::{collections::HashSet, sync::Mutex};

use chrono::{Local, Utc};
use suite_223b::{
    auth::CredentialManager,
    calendar::utils::{CalDavEvent, CalEventType, structs::EventFilter},
};

pub struct EventCache {
    pub timed: Vec<CalDavEvent>,
    pub allday: Vec<CalDavEvent>,
}
impl EventCache {
    pub fn new() -> Self {
        Self {
            timed: Vec::new(),
            allday: Vec::new(),
        }
    }
}

pub struct CalendarBackend {
    pub cache: Mutex<EventCache>,
}
impl CalendarBackend {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(EventCache::new()),
        }
    }

    pub fn get_events_with_filter(&self, filter: EventFilter) -> Vec<CalDavEvent> {
        let Ok(cache) = self.cache.lock() else {
            return vec![];
        };

        let now = Utc::now();

        match filter {
            EventFilter::Today { include_allday } => {
                if include_allday {
                    cache
                        .allday
                        .iter()
                        .chain(cache.timed.iter())
                        .cloned()
                        .collect()
                } else {
                    cache.timed.clone()
                }
            }
            EventFilter::Nearby {
                look_back,
                look_ahead,
            } => {
                // Convert Durations to Chrono Durations
                let past_limit = now
                    - chrono::Duration::from_std(look_back)
                        .unwrap_or_else(|_| chrono::Duration::zero());
                let future_limit = now
                    + chrono::Duration::from_std(look_ahead)
                        .unwrap_or_else(|_| chrono::Duration::zero());

                cache
                    .timed
                    .iter()
                    .filter(|event| {
                        if let Some(start_time) = event.start_utc() {
                            start_time >= past_limit && start_time <= future_limit
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect()
            }
        }
    }

    pub async fn fetch_for_today(&mut self) -> usize {
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
            let cache = self.cache.lock().expect("Failed to read mutex");
            let timed = &cache.timed;
            let allday = &cache.allday;

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
            events.retain(|e| e.occurs_on_day(&today));

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
                let mut cache = self.cache.lock().expect("Failed to lock mutex");
                cache.timed.extend(new_timed);
                cache.allday.extend(new_allday);
            }

            // let _ = self.save_to_cache();
        }
        num_changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::cell::Cell;
    use std::time::Duration;
    use suite_223b::calendar::utils::structs::DateTimeSpec;

    // Helper to create a dummy event
    fn create_test_event(
        uid: &str,
        title: &str,
        is_allday: bool,
        start_offset_mins: i64,
    ) -> CalDavEvent {
        let start_time = Utc::now() + chrono::Duration::minutes(start_offset_mins);

        CalDavEvent {
            uid: uid.to_string(),
            title: title.to_string(),
            event_type: if is_allday {
                CalEventType::AllDay
            } else {
                CalEventType::Timed
            },
            start: Some(DateTimeSpec::DateTime { value: start_time }),
            seen: Cell::new(false),
            ..Default::default()
        }
    }

    #[test]
    fn test_filter_today_logic() {
        let backend = CalendarBackend::new();
        let ev1 = create_test_event("1", "Morning Standup", false, -60);
        let ev2 = create_test_event("2", "All Day Holiday", true, 0);

        {
            let mut cache = backend.cache.lock().unwrap();
            cache.timed.push(ev1);
            cache.allday.push(ev2);
        }

        // Test Today with All Day
        let results = backend.get_events_with_filter(EventFilter::Today {
            include_allday: true,
        });
        assert_eq!(results.len(), 2);

        // Test Today without All Day
        let results_no_allday = backend.get_events_with_filter(EventFilter::Today {
            include_allday: false,
        });
        assert_eq!(results_no_allday.len(), 1);
        assert_eq!(results_no_allday[0].uid, "1");
    }

    #[test]
    fn test_filter_nearby_logic() {
        let backend = CalendarBackend::new();

        // Event started 15 mins ago
        let past_ev = create_test_event("past", "Just Started", false, -15);
        // Event starting in 10 mins
        let future_ev = create_test_event("future", "Starting Soon", false, 10);
        // Event way in the future
        let way_future_ev = create_test_event("far", "Next Week", false, 10000);

        {
            let mut cache = backend.cache.lock().unwrap();
            cache.timed.extend(vec![past_ev, future_ev, way_future_ev]);
        }

        // Filter: Look back 20m, Look ahead 20m
        let filter = EventFilter::Nearby {
            look_back: Duration::from_secs(20 * 60),
            look_ahead: Duration::from_secs(20 * 60),
        };

        let results = backend.get_events_with_filter(filter);

        // Should find "past" and "future" but not "far"
        assert_eq!(results.len(), 2);
        let uids: Vec<String> = results.into_iter().map(|e| e.uid).collect();
        assert!(uids.contains(&"past".into()));
        assert!(uids.contains(&"future".into()));
    }

    #[test]
    fn test_cache_deduplication_logic() {
        let backend = CalendarBackend::new();
        let uid = "unique-123";

        // 1. Pre-populate the cache with an event (The "Old" event)
        {
            let mut cache = backend.cache.lock().unwrap();
            cache
                .timed
                .push(create_test_event(uid, "Original Event", false, 0));
        }

        // 2. Simulate what happens inside fetch_for_today
        // We get a "new" batch of events from a provider
        let incoming_events = vec![
            create_test_event(uid, "Original Event (Duplicate)", false, 0), // Duplicate UID
            create_test_event("unique-456", "New Event", false, 10),        // New UID
        ];

        // 3. This is the logic from your fetch_for_today function, isolated:
        let mut new_timed = Vec::new();
        let mut new_allday = Vec::new();

        // Re-create the seen_ids set just like your function does
        let seen_ids: HashSet<String> = {
            let cache = backend.cache.lock().unwrap();
            cache
                .timed
                .iter()
                .chain(cache.allday.iter())
                .map(|e| e.uid.clone())
                .collect()
        };

        for item in incoming_events {
            if !seen_ids.contains(&item.uid) {
                item.seen.set(false);
                match item.event_type {
                    CalEventType::Timed => new_timed.push(item),
                    CalEventType::AllDay => new_allday.push(item),
                }
            }
        }

        // 4. Verify results
        assert_eq!(
            new_timed.len(),
            1,
            "Only the new event should be staged for update"
        );
        assert_eq!(new_timed[0].uid, "unique-456");

        // 5. Update the cache
        let num_changes = new_timed.len() + new_allday.len();
        if num_changes > 0 {
            let mut cache = backend.cache.lock().unwrap();
            cache.timed.extend(new_timed);
            cache.allday.extend(new_allday);
        }

        // 6. Final check: Cache should have exactly 2 events
        let final_cache = backend.cache.lock().unwrap();
        assert_eq!(final_cache.timed.len(), 2);
    }
}
