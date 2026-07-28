//! Utilities for Tab Completion
//!
//! © Stephen Marz
//! 8 June 2026
use super::COMMAND_TABLE;
use crate::cache::Tree;
use crate::repl::State;
use rustyline::{
    Helper, completion::Completer, highlight::Highlighter, hint::Hinter, validate::Validator,
};

impl Helper for State {}
impl Validator for State {}
impl Highlighter for State {}
impl Hinter for State {
    type Hint = String;

    fn hint(&self, line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() {
            return None;
        }
        line.split_whitespace().next().and_then(|prefix| {
            COMMAND_TABLE
                .iter()
                .find(|cmd| cmd.name.starts_with(prefix))
                .map(|cmd| cmd.name[prefix.len()..].to_string())
        })
    }
}
impl Completer for State {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if line[..pos].contains(' ') {
            // TODO: Make this command specific.
            let cwd = self.cwd.clone();
            if let Some(tree) = self.fs.get_tree() {
                return complete(tree, cwd, line, pos);
            }
            return Ok((pos, Vec::new()));
        }
        let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
        let prefix = &line[start..pos];
        let candidates = COMMAND_TABLE
            .iter()
            .filter(|cmd| cmd.name.starts_with(prefix))
            .map(|cmd| cmd.name.to_string())
            .collect();
        Ok((start, candidates))
    }
}

fn complete(
    tree: &Tree,
    cwd: Vec<String>,
    line: &str,
    pos: usize,
) -> rustyline::Result<(usize, Vec<String>)> {
    let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
    let prefix = &line[start..pos];
    let mut path = if !prefix.starts_with('/') {
        cwd.iter().map(|x| x.as_str()).collect::<Vec<&str>>()
    }
    else {
        vec![]
    };
    path.append(
        &mut prefix
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>(),
    );
    let mut at = tree;
    let end = if path.is_empty() {
        ""
    }
    else {
        path[path.len() - 1]
    };
    let mut spath = String::new();

    path.iter().for_each(|component| {
        if let Some(next) = at.iter().find(|item| item.name() == component) {
            if !next.name().eq(".") && !next.name().eq("..") && next.is_dir() {
                if let Some(t) = next.next() {
                    spath += "/";
                    spath += next.name();
                    at = t;
                }
                else {
                    eprintln!("Error in sub-tree: {}.", next.name());
                }
            }
        }
    });

    let mut ret = vec![];
    at.iter().for_each(|component| {
        if (component.name() != "." && component.name() != "..")
            && (end.is_empty() || spath.ends_with(end) || component.name().starts_with(end))
        {
            ret.push(spath.clone() + "/" + &component.clone_name());
        }
    });
    Ok((start, ret))
}
