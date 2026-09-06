//! ZDEP-023/ZDEP-024: ConPTY-only, zero-third-party-dependency port of
//! portable-pty-psmux 0.9.7 (MIT) win/ code, folded from
//! crates/portable-pty-psmux into the root crate. `Error` is a direct alias
//! for `std::io::Error` (A7) rather than `anyhow::Error`; nothing downcasts
//! a pty trait object, so the `downcast-rs` supertraits are dropped too.

mod child;
mod cmdbuilder;
pub mod conpty;
pub(crate) mod ffi;
mod handle;
mod procthreadattr;
mod psuedocon;
mod registry;

pub use cmdbuilder::CommandBuilder;
// Only referenced from tests-rs/test_zdep_pty_fold.rs and friends (included
// as cfg(test) modules): unused outside a test build.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use cmdbuilder::append_quoted;
#[cfg_attr(not(test), allow(unused_imports))]
pub use psuedocon::{
    conpty_base_flags, passthrough_supported, probe_conpty, PSEUDOCONSOLE_PASSTHROUGH_MODE,
    PSEUDOCONSOLE_RESIZE_QUIRK, PSEUDOCONSOLE_WIN32_INPUT_MODE,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use registry::registry_environment;

/// `crate::pty::Error` IS `std::io::Error` (A7): no downcast is ever needed
/// to inspect an error a pty operation returned.
pub type Error = std::io::Error;

/// Represents the size of the visible display area in the pty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    /// The number of lines of text
    pub rows: u16,
    /// The number of columns of text
    pub cols: u16,
    /// The width of a cell in pixels. Note that some systems never fill
    /// this value and ignore it.
    pub pixel_width: u16,
    /// The height of a cell in pixels. Note that some systems never fill
    /// this value and ignore it.
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
    }
}

/// Represents the master/control end of the pty
pub trait MasterPty: Send {
    /// Inform the kernel and thus the child process that the window resized.
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    /// Retrieves the size of the pty as known by the kernel
    fn get_size(&self) -> Result<PtySize, Error>;
    /// Obtain a readable handle; output from the slave(s) is readable via
    /// this stream.
    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error>;
    /// Obtain a writable handle; writing to it will send data to the slave
    /// end. Dropping the writer will send EOF to the slave end. It is
    /// invalid to take the writer more than once.
    fn take_writer(&self) -> Result<Box<dyn std::io::Write + Send>, Error>;

    /// Whether this is a ConPTY created with passthrough mode enabled.
    ///
    /// `None` means that the pty implementation is not ConPTY, or does not
    /// expose this implementation-specific detail. The result should be
    /// queried after spawning a child: ConPTY can fall back to a
    /// newly-created non-passthrough console when process creation rejects
    /// passthrough mode.
    fn conpty_passthrough_mode(&self) -> Option<bool> {
        None
    }
}

/// Represents a child process spawned into the pty. This handle can be used
/// to wait for or terminate that child process.
pub trait Child: std::fmt::Debug + ChildKiller + Send {
    /// Poll the child to see if it has completed. Does not block. Returns
    /// None if the child has not yet terminated, else returns its exit
    /// status.
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
    /// Blocks execution until the child process has completed, yielding its
    /// exit status.
    fn wait(&mut self) -> std::io::Result<ExitStatus>;
    /// Returns the process identifier of the child process, if applicable
    fn process_id(&self) -> Option<u32>;
    /// Returns the process handle of the child process, if applicable.
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle>;
}

/// Represents the ability to signal a Child to terminate
pub trait ChildKiller: std::fmt::Debug + Send {
    /// Terminate the child process
    fn kill(&mut self) -> std::io::Result<()>;
    /// Clone an object that can be split out from the Child in order to
    /// send it signals independently from a thread that may be blocked in
    /// `.wait`.
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync>;
}

/// Represents the slave side of a pty. Can be used to spawn processes into
/// the pty.
pub trait SlavePty {
    /// Spawns the command specified by the provided CommandBuilder
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<dyn Child + Send + Sync>, Error>;
}

/// Represents the exit status of a child process.
#[derive(Debug, Clone)]
pub struct ExitStatus {
    code: u32,
}

impl ExitStatus {
    /// Construct an ExitStatus from a process return code
    pub fn with_exit_code(code: u32) -> Self {
        Self { code }
    }

    /// Returns true if the status indicates successful completion
    pub fn success(&self) -> bool {
        self.code == 0
    }

    /// Returns the exit code that this ExitStatus was constructed with
    pub fn exit_code(&self) -> u32 {
        self.code
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.success() {
            write!(fmt, "Success")
        } else {
            write!(fmt, "Exited with code {}", self.code)
        }
    }
}

pub struct PtyPair {
    // slave is listed first so that it is dropped first. The drop order is
    // stable and specified by rust rfc 1857.
    pub slave: Box<dyn SlavePty + Send>,
    pub master: Box<dyn MasterPty + Send>,
}

/// The `PtySystem` trait allows an application to work with multiple
/// possible Pty implementations at runtime.
pub trait PtySystem {
    /// Create a new Pty instance with the window size set to the specified
    /// dimensions. Returns a (master, slave) Pty pair. The master side is
    /// used to drive the slave side.
    fn openpty(&self, size: PtySize) -> Result<PtyPair, Error>;
}

pub fn native_pty_system() -> Box<dyn PtySystem + Send> {
    Box::new(conpty::ConPtySystem::default())
}

/// Serializes process-global console identity changes against ConPTY spawns.
///
/// On Windows, FreeConsole/AttachConsole swap the whole process's console
/// connection and its std handle slots. CreateProcessW for a ConPTY child
/// (bInheritHandles=FALSE, no STARTF_USESTDHANDLES) stamps the parent's std
/// handle *values* into the child's ProcessParameters at that instant; a
/// spawn landing inside another thread's FreeConsole/AttachConsole window
/// gives the child freed, recycled handle values and the shell dies at its
/// first console read (psmux issue #450). Every FreeConsole/AttachConsole
/// dance and every ConPTY spawn must hold this lock.
pub fn console_state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A panic while holding the lock leaves console state possibly odd but
    // the lock itself must keep working (see psmux issue #446).
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
