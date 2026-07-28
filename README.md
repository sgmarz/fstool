# File System Tool (fstool)

An interactive tool to create, read, or write file systems.

## Current Filesystems

* Minix 3
* Ext2 - in progress (0/100)

## Source File

When running the tool, a source file will try to be read. This file will have a command line-by-line. Any command that cannot be executed will give a warning, but the file will be continue to be read. In this file, comments are specified by `#` or `//` and there are only line-comments.

The default name `$HOME/.config/fstoolrc` can be overridden by the `-s/--source` switch.

### Example Source File (fstoolrc)

```shell
echo Creating aliases...
# Alias ll as ls -l
alias ll ls -l
// Alias la as ls -a
alias la ls -a
# The break command stops futher evaluation of the file.
break
# This will not be evaluated since it comes after break.
alias lla ls -l -a
```

The command `break` will stop reading the file. This has little utility, but it allows you to incrementally add things to the source file.

## Switches

* `-c/--create` - Create a new file system of the given type.
* `-z/--size` - Used with create to give the size of the file to create.
* `-s/--source` - Source the given file at the start. Defaults to: `$HOME/.config/fstoolrc`.
* `-f/--fs-type` - File system to create. Defaults to: `minix`.
* `-w/--no-write` - Open the file system image read-only.
* `-n/--no-color` - Do not display colors. This is typically used force `ls -c`, which disables colors for the ls command.
* `-u/--uid` - Specify the creation UID. Defaults to 0 (root).
* `-g/--gid` - Specify the creation GID. Defaults to 0 (root).
* `-k/--umask` - Specify the creation mask. Defaults to `022`. Must be in octal (base 8).
* `-h/--help` - Prints the command line argument help.
* `-V/--version` - Prints the version and exits.

### Usage

```sh
fstool [switches] <file system image>
```

## Creating a File System

## Interactive Commands

## Examples

```sh
~> fstool hdd.bin
```
