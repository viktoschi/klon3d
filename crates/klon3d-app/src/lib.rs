//! klon3d-app – the application window of stage 6 (see `README.md` in this crate).
//!
//! | Module | Content |
//! |---|---|
//! | [`app`] | `App` (`eframe::App`): state, actions, shortcuts, dialogs, frame layout |
//! | [`menue`] | menu bar and toolbar |
//! | [`panele`] | side panels (pieces, seams, properties) and status bar |
//! | [`ansicht2d`] | the 2D view and its view math |
//! | [`dateiwahl`] | the in-app file dialog |
//! | [`muster`] | a loaded pattern as plain data, no egui types |
//!
//! The modules are public so that integration tests can build the window without a real one.

pub mod ansicht2d;
pub mod app;
pub mod dateiwahl;
pub mod menue;
pub mod muster;
pub mod panele;

pub use app::App;
