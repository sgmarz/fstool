//! Command Table
//!
//! © Stephen Marz
//! 8 June 2026
use super::Command;

pub const COMMAND_TABLE: &[Command] = &[
    Command::new(
        "help",
        "Show this help message. help <topic> for more information.",
        "",
        super::help::do_help,
    ),
    Command::new("exit", "Exit the shell.", "", |state, _args| state.stop()),
    Command::new("quit", "Alias for \"exit\".", "", |state, _args| {
        state.stop()
    }),
    Command::new(
        "source",
        "Source a file to run commands line-by-line.",
        "<path>",
        super::shell::do_source,
    ),
    Command::new(
        "cd",
        "Change the current working directory.",
        "<path>",
        super::dir::do_chdir,
    ),
    Command::new(
        "cat",
        "Print the contents of a file to the terminal.",
        "<path>",
        super::cat::do_cat,
    ),
    Command::new(
        "catsymlink",
        "Print the contents of a symbolic link.",
        "<path>",
        super::cat::do_catsymlink,
    ),
    Command::new(
        "catblock",
        "Print the contents of a block.",
        "<block number>",
        super::cat::do_catblock,
    ),
    Command::new(
        "head",
        "Print the first set of bytes of a file to the terminal.",
        "<path>",
        super::cat::do_head,
    ),
    Command::new(
        "hexdump",
        "Print the contents of a file in hexadecimal.",
        "<path>",
        super::cat::do_hexdump,
    ),
    Command::new(
        "randbytes",
        "Write random bytes to a file.",
        "<size> <path>",
        super::misc::do_randomb,
    ),
    Command::new(
        "randtext",
        "Write random ASCII text to a file.",
        "<size> <path>",
        super::misc::do_randomt,
    ),
    Command::new(
        "touch",
        "Create a new, empty file or update access times.",
        "<file>",
        super::misc::do_touch,
    ),
    Command::new(
        "chmod",
        "Change the permissions or mode.",
        "<octal mode> <path>",
        super::misc::do_chmod,
    ),
    Command::new(
        "chown",
        "Change the owner and/or group of a file.",
        "<owner>[:<group>] <path>",
        super::misc::do_chown,
    ),
    Command::new(
        "cp",
        "Copy a file or directory within the image.",
        "<from path> <to path>",
        super::xfer::do_cp,
    ),
    Command::new(
        "mv",
        "Move a file or directory within the image.",
        "<from path> <to path>",
        super::xfer::do_mv,
    ),
    Command::new("rm", "Remove a file.", "[ifr] <path>", super::rm::do_rm),
    Command::new(
        "rmdir",
        "Remove an empty directory.",
        "<path>",
        super::dir::do_rmdir,
    ),
    Command::new(
        "get",
        "Copy a file from the image to the local machine.",
        "<remote path> <local path>",
        super::xfer::do_get,
    ),
    Command::new(
        "put",
        "Copy a file from the local machine to the image.",
        "<local path> <remote path>",
        super::xfer::do_put,
    ),
    Command::new(
        "mkdir",
        "Create a directory in the filesystem.",
        "<path>",
        super::dir::do_mkdir,
    ),
    Command::new(
        "mknod",
        "Create a node in the filesystem.",
        "<name> <type> <major> <minor>\n         - where type is b (block), c (char), p (pipe/FIFO), or s (socket)",
        super::misc::do_mknod,
    ),
    Command::new(
        "pwd",
        "Print the current working directory.",
        "",
        |state, _args| {
            println!("/{}", state.cwd.join("/"));
        },
    ),
    Command::new(
        "ls",
        "List the contents of the current directory.",
        "",
        super::ls::do_ls,
    ),
    Command::new(
        "ln",
        "Create a link in the filesystem.",
        "[-sf] <target path> <link path>",
        super::misc::do_ln,
    ),
    Command::new(
        "df",
        "Get free/taken inodes and blocks.",
        "",
        super::misc::do_df,
    ),
    Command::new(
        "du",
        "Get the disk usage for a given directory.",
        "-[hPscdNtN] [directory]",
        super::stat::do_du,
    ),
    Command::new(
        "stat",
        "Get information about a file or directory.",
        "<path>",
        super::stat::do_stat,
    ),
    Command::new(
        "istat",
        "Get information about an inode by number.",
        "<number>",
        super::stat::do_istat,
    ),
    Command::new(
        "save",
        "Save the current state of the filesystem to the image file.",
        "",
        super::xfer::do_save,
    ),
    Command::new(
        "shell",
        "Spawn a shell and suspend this program.",
        "",
        super::shell::do_shell,
    ),
    Command::new(
        "exec",
        "Execute a command on the local machine and cat the given file into its stdin.",
        "<path> <command> [command arguments]",
        super::shell::do_exec,
    ),
    Command::new(
        "umask",
        "Set the umask for new files and directories.",
        "<octal mode>",
        super::misc::do_umask,
    ),
    Command::new(
        "uid",
        "Get/set the user ID for new files and directories.",
        "[uid]",
        super::misc::do_uid,
    ),
    Command::new(
        "gid",
        "Get/set the group ID for new files and directories.",
        "[gid]",
        super::misc::do_gid,
    ),
    Command::new(
        "echo",
        "Output the arguments to the screen.",
        "[output]",
        super::shell::do_echo,
    ),
    Command::new(
        "alias",
        "View, create, or delete aliases.",
        "[name] [command]",
        super::alias::do_alias,
    ),
    Command::new(
        "remount-ro",
        "Remount the filesystem read-only.",
        "",
        super::misc::do_remount_ro,
    ),
];
