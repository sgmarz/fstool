//! Alias Command
//!
//! © Stephen Marz
//! 8 June 2026
use super::Args;
use super::State;

pub(super) fn do_alias(state: &mut State, args: Args) {
    // If no arguments, print the aliases.
    if args.len() == 0 {
        if state.aliases.is_empty() {
            println!("No aliases.");
            return;
        }
        println!("Alias        Command");
        println!("~~~~~        ~~~~~~~");
        let mut alias_table: Vec<(&str, &str)> = state
            .aliases
            .iter()
            .map(|(alias, command)| (alias.as_str(), command.as_str()))
            .collect();
        alias_table.sort_by(|a, b| {
            let a_alias = a.0;
            let b_alias = b.0;
            match a_alias.cmp(b_alias) {
                std::cmp::Ordering::Equal => a.1.cmp(b.1),
                x => x,
            }
        });
        alias_table.iter().for_each(|(alias, command)| {
            println!("{:<12} {}", alias, command);
        });
    }
    // If there is one, that signals to delete the given alias.
    else if args.len() == 1 {
        let alias = args[0];
        if let Some(_) = state.aliases.remove(alias) {
            println!("{}: alias deleted.", alias);
        }
        else {
            println!("{}: alias not found", alias);
        }
    }
    else {
        let alias = args[0];
        let command = args[1..].join(" ");
        state.aliases.insert(alias.to_string(), command);
    }
}
