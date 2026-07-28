//! REPL library structures and modules
//!
//! © Stephen Marz
//! 8 June 2026
pub use cmdtable::COMMAND_TABLE;
pub use state::State;
pub use user::{prompt_overwrite, prompt_with, repl, run};

mod completer;
mod user;
// Command table for REPL
mod cmdtable;

// Commands in submodules
mod alias;
mod cat;
mod dir;
mod help;
mod ls;
mod misc;
mod rm;
mod shell;
mod stat;
pub(super) mod state;
mod xfer;

pub type Args<'a> = &'a [&'a str];
pub type Handler = fn(&mut State, Args);

pub struct Command<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub usage: &'a str,
    pub handler: Handler,
}

impl<'a> Command<'a> {
    pub const fn new(
        name: &'a str,
        description: &'a str,
        usage: &'a str,
        handler: Handler,
    ) -> Self {
        Self {
            name,
            description,
            usage,
            handler,
        }
    }
}
