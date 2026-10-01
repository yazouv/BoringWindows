//! Agenda : calendriers ICS (Google, Outlook, iCloud, Proton…), prochaines
//! réunions, rappel avant le début et lien « Rejoindre ».

mod agenda;
mod config;
mod fetch;
pub mod ics;
mod join;
mod module;
mod tz;

pub use agenda::{AgendaItem, CalendarSnapshot};
pub use config::{CalendarConfig, Source};
pub use ics::Event;
pub use module::{CalendarModule, MODULE_ID};
