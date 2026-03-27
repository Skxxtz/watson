use std::{io::Cursor, sync::Arc};

use ical::IcalParser;

use crate::calendar::utils::{CalDavEvent, CalendarInfo};

pub fn unfold_ics(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }

            match chars.peek() {
                Some(' ') | Some('\t') => {
                    chars.next(); // swallow folding whitespace
                }
                _ => out.push('\n'),
            }
        } else if c == '\n' {
            match chars.peek() {
                Some(' ') | Some('\t') => {
                    chars.next(); // swallow folding whitespace
                }
                _ => out.push('\n'),
            }
        } else {
            out.push(c);
        }
    }

    out
}

pub fn parse_ical(ics: String, calendar_info: Arc<CalendarInfo>) -> Vec<CalDavEvent> {
    let parser = IcalParser::new(Cursor::new(&ics));
    let mut out = Vec::new();

    for calendar in parser {
        let calendar = match calendar {
            Ok(c) => c,
            Err(_) => continue,
        };

        for event in calendar.events {
            match CalDavEvent::try_from(event) {
                Ok(mut ev) => {
                    ev.calendar_info = calendar_info.clone();
                    out.push(ev);
                }
                Err(e) => {
                    eprint!("{:?}", e)
                }
            }
        }
    }

    out
}
