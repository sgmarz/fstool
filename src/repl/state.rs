//! Internal Tool State
//!
//! © Stephen Marz
//! 8 June 2026
use crate::fs::FileSystem;
use rustyline::{Editor, history::FileHistory};
use std::{collections::HashMap, io, path::PathBuf};

pub struct State {
    pub run: bool,
    pub write: bool,
    pub cwd: Vec<String>,
    pub filename: PathBuf,
    pub fs: Box<dyn FileSystem>,
    pub changes: bool,
    pub no_color: bool,
    pub uid: u32,
    pub gid: u32,
    pub umask: u16,
    pub aliases: HashMap<String, String>,
}

impl State {
    pub fn new(
        filename: PathBuf,
        write: bool,
        no_color: bool,
        uid: u32,
        gid: u32,
        umask: u16,
        fs: Box<dyn FileSystem>,
    ) -> io::Result<Editor<State, FileHistory>> {
        let mut rl_editor: Editor<State, FileHistory> =
            Editor::new().expect("unable to create readline editor");
        rl_editor.set_helper(Some(State {
            run: true,
            write,
            cwd: vec![],
            filename,
            fs,
            changes: false,
            no_color,
            uid,
            gid,
            umask,
            aliases: HashMap::new(),
        }));
        Ok(rl_editor)
    }

    pub fn changed(&mut self) {
        self.changes = true;
    }

    pub fn reset_changed(&mut self) {
        self.changes = false;
    }

    pub fn stop(&mut self) {
        self.run = false;
    }

    pub fn is_running(&self) -> bool {
        self.run
    }

    pub fn apply_umask_to(&self, mode: u16) -> u16 {
        mode & !self.umask
    }

    pub fn file_umask(&self) -> u16 {
        self.apply_umask_to(0o666)
    }

    pub fn dir_umask(&self) -> u16 {
        self.apply_umask_to(0o777)
    }
}
