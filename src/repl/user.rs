//! User Interface (REPL and Commands)
//!
//! © Stephen Marz
//! 8 June 2026
use super::{COMMAND_TABLE, State};
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::{Editor, history::FileHistory};
use std::io::{self, Write};

pub fn repl(rl_editor: &mut Editor<State, FileHistory>) {
    // Interactive shell loop. We print the prompt, read a line of input, and run the command. If the command returns an error or if the user enters an empty line, we exit the loop.
    'mainloop: loop {
        if !prompt(rl_editor) || !rl_editor.helper().unwrap().is_running() {
            let state = rl_editor.helper_mut().unwrap();
            if state.changes && state.write {
                let mut input = String::new();
                loop {
                    input.clear();
                    print!("Changes were made, save (y/n)? ");
                    io::stdout().flush().unwrap();
                    if io::stdin().read_line(&mut input).is_err()
                        || (input.trim().to_lowercase() != "y"
                            && input.trim().to_lowercase() != "n")
                    {
                        eprintln!("Please enter 'y' or 'n'.");
                    }
                    else if input.trim().to_lowercase() == "y"
                        && let Err(r) = state.fs.write_to_backing()
                    {
                        eprintln!("Failed to write changes back to disk: {}", r);
                    }
                    else {
                        break 'mainloop;
                    }
                }
            }
            else {
                // No changes or read-only, just exit.
                break;
            }
        }
        println!();
    }
}

fn prompt(rl_editor: &mut Editor<State, FileHistory>) -> bool {
    let state = rl_editor.helper().unwrap();
    let mut prompt = format!("{}:/", state.filename.display());
    if !state.cwd.is_empty() {
        prompt += format!("{}", state.cwd.join("/")).as_str();
    }
    prompt += "$ ";
    match rl_editor.readline(&prompt) {
        Ok(line) if line.eq("history") => {
            rl_editor
                .history()
                .iter()
                .enumerate()
                .for_each(|(idx, entry)| {
                    println!("{}: {}", idx + 1, entry);
                });
        }
        Ok(line) if line.eq("history -d") => {
            let _ = rl_editor.history_mut().clear();
        }
        Ok(line) => {
            let state = rl_editor.helper_mut().unwrap();
            match run(state, line.as_str()) {
                true => {
                    let _ = rl_editor.add_history_entry(line);
                }
                false => {
                    println!("Unknown command: '{line}'");
                }
            }
        }
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
            return false;
        }
        Err(err) => {
            eprintln!("Error reading line: {}", err);
            return false;
        }
    }
    true
}

pub fn run(state: &mut State, command: &str) -> bool {
    let args = command.split_whitespace().collect::<Vec<&str>>();
    if args.is_empty() {
        return true;
    }

    // Alias resolution
    //
    // Build an owned Vec<String> for the effective argument list so that:
    //   a) it outlives the alias lookup (no borrow of state.aliases kept alive), and
    //   b) the mutable borrow of state passed to the handler has no conflicts.
    //
    // Alias expansion mirrors BASH: the alias value is split into words, then
    // any extra arguments from the original command line are appended.
    // e.g. alias ll="ls -l" + command "ll -a" is effectively ["ls", "-l", "-a"]
    let mut is_alias = false;
    let effective: Vec<String> = match state.aliases.get(args[0]) {
        Some(alias_val) => {
            let mut expanded: Vec<String> =
                alias_val.split_whitespace().map(str::to_owned).collect();
            // Append any extra args the user typed after the alias name.
            expanded.extend(args[1..].iter().map(|s| s.to_string()));
            is_alias = true;
            expanded
        }
        // No alias found — use the original command words as-is.
        None => args.iter().map(|s| s.to_string()).collect(),
    };

    if effective.is_empty() {
        return true;
    }

    // Borrow &str views from the owned Vec.  These are valid for the rest of
    // the function since `effective` is not moved or dropped until after the
    // handler returns.
    let match_to: &str = &effective[0];
    let parameters: Vec<&str> = effective[1..].iter().map(String::as_str).collect();

    // Dispatch command
    for cmd in COMMAND_TABLE {
        if cmd.name == match_to {
            (cmd.handler)(state, &parameters);
            return true;
        }
    }
    if is_alias {
        eprintln!(
            "Alias '{}' expands to unknown command '{}'.",
            args[0], match_to
        );
        return true;
    }
    false
}

/// Used in several places for confirmation.
/// Prompt "overwrite '<path>'? [y/N] " and return true iff the user answers
/// "y" or "yes".  Any I/O error is treated as "no".
pub fn prompt_overwrite(path: &str) -> bool {
    prompt_with("overwrite", path)
}

/// Prompt with the given prompt, wait for the user to type y or yes.
/// Returns true if the user confirms, false otherwise.
/// Anything other than y/yes is considered a no.
///
/// Writes as: "prompt 'path'? [y/N] "
pub fn prompt_with(prompt: &str, path: &str) -> bool {
    use std::io::{BufRead, Write};
    print!("{prompt} '{path}'? [y/N] ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).ok();
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
