use super::{completer::IonCompleter, InteractiveShell};
use ion_shell::Shell;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use std::io::ErrorKind;

impl<'a> InteractiveShell<'a> {
    /// Make sure to reset the fd to blocking mode
    fn change_blocking(fd: std::os::unix::io::RawFd) {
        // Get current flags, clear O_NONBLOCK, set back
        // Note: O_RDWR is an access mode, not a status flag - can't use it to clear O_NONBLOCK
        if let Ok(flags) = fcntl(fd, FcntlArg::F_GETFL) {
            let mut oflags = OFlag::from_bits_truncate(flags);
            oflags.remove(OFlag::O_NONBLOCK);
            let _ = fcntl(fd, FcntlArg::F_SETFL(oflags));
        }
    }

    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    pub fn readln<T: Fn(&mut Shell<'_>)>(&self, prep_for_exit: &T) -> Option<String> {
        Self::change_blocking(0);
        Self::change_blocking(1);
        Self::change_blocking(2);
        let prompt = self.prompt();
        let line = self.context.borrow_mut().read_line(
            prompt,
            None,
            &mut IonCompleter::new(&self.shell.borrow()),
        );

        match line {
            Ok(line) => {
                if line.bytes().next() != Some(b'#')
                    && line.bytes().any(|c| !c.is_ascii_whitespace())
                {
                    self.terminated.set(false);
                }
                Some(line)
            }
            // Handles Ctrl + C
            Err(ref err) if err.kind() == ErrorKind::Interrupted => None,
            // Handles Ctrl + D
            Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => {
                let mut shell = self.shell.borrow_mut();
                if self.terminated.get() && shell.exit_block().is_err() {
                    prep_for_exit(&mut shell);
                    std::process::exit(shell.previous_status().as_os_code())
                }
                None
            }
            Err(err) => {
                eprintln!("ion: liner: {}", err);
                None
            }
        }
    }
}
