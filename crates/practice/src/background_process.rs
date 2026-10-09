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
    let mut command = Command::new(crate::tool_setup::interpreter(program).map_or_else(|| program.to_os_string(), |path| path.into_os_string()));
    hide_console(&mut command);
    command
}
