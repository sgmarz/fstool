//! ANSI Terminal Constants and Colors
//!
//! © Stephen Marz
//! 8 June 2026
#![allow(dead_code)]
pub const ANSI_NORMAL: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_UNDERLINE: &str = "\x1b[4m";
pub const ANSI_REVERSED: &str = "\x1b[7m";

// Bright colors
pub mod bright {
    pub const BLACK: &str = "\x1b[90m";
    pub const RED: &str = "\x1b[91m";
    pub const GREEN: &str = "\x1b[92m";
    pub const YELLOW: &str = "\x1b[93m";
    pub const BLUE: &str = "\x1b[94m";
    pub const MAGENTA: &str = "\x1b[95m";
    pub const CYAN: &str = "\x1b[96m";
    pub const WHITE: &str = "\x1b[97m";
}

// Normal colors
pub mod normal {
    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
}

// Background colors
pub mod background {
    pub const BLACK: &str = "\x1b[40m";
    pub const RED: &str = "\x1b[41m";
    pub const GREEN: &str = "\x1b[42m";
    pub const YELLOW: &str = "\x1b[43m";
    pub const BLUE: &str = "\x1b[44m";
    pub const MAGENTA: &str = "\x1b[45m";
    pub const CYAN: &str = "\x1b[46m";
    pub const WHITE: &str = "\x1b[47m";
}
