//! Background child processes must not create Windows console windows.
use std::process::Command;

pub fn hide_console(command: &mut Command) {
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))] let _ = command;
}

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let program = program.as_ref();
    let mut command = if let Some((binary, args)) = crate::tool_setup::managed_command(program) {
        let mut command = Command::new(binary); command.args(args); command
    } else { Command::new(program) };
    crate::tool_setup::environment(&mut command);
    hide_console(&mut command);
    command
}
