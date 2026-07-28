//! Help Command
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, COMMAND_TABLE, State};

pub(super) fn do_help(_state: &mut State, args: Args) {
    if args.is_empty() {
        println!("All commands:");
        for cmd in COMMAND_TABLE {
            println!("  {:<10} - {}", cmd.name, cmd.description);
        }
    }
    else {
        println!("Commands starting with '{}':", args[0]);
        let mut found = 0;
        for cmd in COMMAND_TABLE {
            if cmd.name.starts_with(args[0]) {
                println!("  {:<10} - {}", cmd.name, cmd.description);
                println!("       Usage: {} {}", cmd.name, cmd.usage);
                found += 1;
            }
        }
        println!(
            "  {} command{} found starting with '{}'",
            found,
            if found == 1 { "" } else { "s" },
            args[0]
        );
    }
}
