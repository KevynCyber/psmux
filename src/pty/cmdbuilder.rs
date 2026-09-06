//! ZDEP-023: port of portable-pty-psmux 0.9.7 (MIT) `src/cmdbuilder.rs`,
//! Windows half only (this crate is ConPTY-only; the unix half is dropped).

use super::registry::registry_environment;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// Used to deal with Windows having case-insensitive environment variables.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
struct EnvEntry {
    /// Whether or not this environment variable came from the base environment,
    /// as opposed to having been explicitly set by the caller.
    is_from_base_env: bool,
    /// The environment variable key in its preferred casing.
    preferred_key: OsString,
    /// The environment variable value.
    value: OsString,
}

impl EnvEntry {
    fn map_key(k: OsString) -> OsString {
        match k.to_str() {
            Some(s) => s.to_lowercase().into(),
            None => k,
        }
    }
}

fn get_base_env() -> BTreeMap<OsString, EnvEntry> {
    let mut env: BTreeMap<OsString, EnvEntry> = std::env::vars_os()
        .map(|(key, value)| {
            (
                EnvEntry::map_key(key.clone()),
                EnvEntry {
                    is_from_base_env: true,
                    preferred_key: key,
                    value,
                },
            )
        })
        .collect();

    for (name, value) in registry_environment() {
        env.insert(
            EnvEntry::map_key(name.clone()),
            EnvEntry {
                is_from_base_env: true,
                preferred_key: name,
                value,
            },
        );
    }

    env
}

/// `CommandBuilder` is used to prepare a command to be spawned into a pty.
/// The interface is intentionally similar to that of `std::process::Command`.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandBuilder {
    args: Vec<OsString>,
    envs: BTreeMap<OsString, EnvEntry>,
    cwd: Option<OsString>,
}

impl CommandBuilder {
    /// Create a new builder instance with argv\[0\] set to the specified
    /// program.
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![program.as_ref().to_owned()],
            envs: get_base_env(),
            cwd: None,
        }
    }

    pub fn from_argv(args: Vec<OsString>) -> Self {
        Self {
            args,
            envs: get_base_env(),
            cwd: None,
        }
    }

    pub fn new_default_prog() -> Self {
        Self {
            args: vec![],
            envs: get_base_env(),
            cwd: None,
        }
    }

    pub fn is_default_prog(&self) -> bool {
        self.args.is_empty()
    }

    /// Append an argument to the current command line.
    /// Will panic if called on a builder created via `new_default_prog`.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) {
        if self.is_default_prog() {
            panic!("attempted to add args to a default_prog builder");
        }
        self.args.push(arg.as_ref().to_owned());
    }

    pub fn args<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
    }

    pub fn get_argv(&self) -> &Vec<OsString> {
        &self.args
    }

    /// Override the value of an environmental variable
    pub fn env<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key: OsString = key.as_ref().into();
        let value: OsString = value.as_ref().into();
        self.envs.insert(
            EnvEntry::map_key(key.clone()),
            EnvEntry {
                is_from_base_env: false,
                preferred_key: key,
                value,
            },
        );
    }

    pub fn env_remove<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.remove(&EnvEntry::map_key(key));
    }

    pub fn env_clear(&mut self) {
        self.envs.clear();
    }

    pub fn get_env<K>(&self, key: K) -> Option<&OsStr>
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.get(&EnvEntry::map_key(key)).map(
            |EnvEntry {
                 is_from_base_env: _,
                 preferred_key: _,
                 value,
             }| value.as_os_str(),
        )
    }

    pub fn cwd<D>(&mut self, dir: D)
    where
        D: AsRef<OsStr>,
    {
        self.cwd = Some(dir.as_ref().to_owned());
    }

    pub fn get_cwd(&self) -> Option<&OsString> {
        self.cwd.as_ref()
    }

    /// Iterate over the configured environment. Only includes environment
    /// variables set by the caller via `env`, not variables set in the base
    /// environment.
    pub fn iter_extra_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.values().filter_map(
            |EnvEntry {
                 is_from_base_env,
                 preferred_key,
                 value,
             }| {
                if *is_from_base_env {
                    None
                } else {
                    let key = preferred_key.to_str()?;
                    let value = value.to_str()?;
                    Some((key, value))
                }
            },
        )
    }

    pub fn iter_full_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.values().filter_map(
            |EnvEntry {
                 preferred_key,
                 value,
                 ..
             }| {
                let key = preferred_key.to_str()?;
                let value = value.to_str()?;
                Some((key, value))
            },
        )
    }

    fn search_path(&self, exe: &OsStr) -> OsString {
        if let Some(path) = self.get_env("PATH") {
            let extensions = self.get_env("PATHEXT").unwrap_or(OsStr::new(".EXE"));
            for path in std::env::split_paths(&path) {
                let candidate = path.join(exe);
                if candidate.exists() {
                    return candidate.into_os_string();
                }
                for ext in std::env::split_paths(&extensions) {
                    let ext = ext.to_str().expect("PATHEXT entries must be utf8");
                    let path = path.join(exe).with_extension(&ext[1..]);
                    if path.exists() {
                        return path.into_os_string();
                    }
                }
            }
        }
        exe.to_owned()
    }

    pub(crate) fn current_directory(&self) -> Option<Vec<u16>> {
        let home: Option<&OsStr> = self
            .get_env("USERPROFILE")
            .filter(|path| Path::new(path).is_dir());
        let cwd: Option<&OsStr> = self.cwd.as_deref().filter(|path| Path::new(path).is_dir());
        let dir: Option<&OsStr> = cwd.or(home);

        dir.map(|dir| {
            let mut wide = vec![];
            if Path::new(dir).is_relative() {
                if let Ok(ccwd) = std::env::current_dir() {
                    wide.extend(ccwd.join(dir).as_os_str().encode_wide());
                } else {
                    wide.extend(dir.encode_wide());
                }
            } else {
                wide.extend(dir.encode_wide());
            }
            wide.push(0);
            wide
        })
    }

    /// Constructs an environment block for this spawn attempt.
    ///
    /// CreateProcessW rejects the WHOLE block with ERROR_INVALID_PARAMETER
    /// (87) if any single entry is malformed (issue #167): an empty name, a
    /// name beginning with '=' (cmd.exe drive vars like "=C:"), or an
    /// interior NUL. The env map is populated from externally-controlled
    /// sources (the parent process environment and the HKLM/HKCU Environment
    /// registry keys), so sanitize rather than let one bad entry poison the
    /// whole spawn.
    pub(crate) fn environment_block(&self) -> Vec<u16> {
        let mut block = vec![];

        for EnvEntry {
            is_from_base_env: _,
            preferred_key,
            value,
        } in self.envs.values()
        {
            let key: Vec<u16> = preferred_key.encode_wide().collect();
            if key.is_empty() || key[0] == b'=' as u16 || key.contains(&0) {
                continue;
            }

            block.extend_from_slice(&key);
            block.push(b'=' as u16);
            // Truncate the value at the first interior NUL rather than drop
            // the whole variable, preserving as much of a usable value as
            // possible.
            block.extend(value.encode_wide().take_while(|&c| c != 0));
            block.push(0);
        }
        block.push(0);

        block
    }

    pub fn get_shell(&self) -> String {
        let exe: OsString = self
            .get_env("ComSpec")
            .unwrap_or(OsStr::new("cmd.exe"))
            .into();
        exe.into_string()
            .unwrap_or_else(|_| "%CompSpec%".to_string())
    }

    pub(crate) fn cmdline(&self) -> std::io::Result<(Vec<u16>, Vec<u16>)> {
        let mut cmdline = Vec::<u16>::new();

        let exe: OsString = if self.is_default_prog() {
            self.get_env("ComSpec")
                .unwrap_or(OsStr::new("cmd.exe"))
                .into()
        } else {
            self.search_path(&self.args[0])
        };

        append_quoted(&exe, &mut cmdline);

        // Ensure that we nul terminate the module name, otherwise we'll
        // ask CreateProcessW to start something random!
        let mut exe: Vec<u16> = exe.encode_wide().collect();
        exe.push(0);

        for arg in self.args.iter().skip(1) {
            cmdline.push(' ' as u16);
            if arg.encode_wide().any(|c| c == 0) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid encoding for command line argument {:?}", arg),
                ));
            }
            append_quoted(arg, &mut cmdline);
        }
        cmdline.push(0);
        Ok((exe, cmdline))
    }
}

// Borrowed from https://github.com/hniksic/rust-subprocess/blob/873dfed165173e52907beb87118b2c0c05d8b8a1/src/popen.rs#L1117
// which in turn was translated from ArgvQuote at http://tinyurl.com/zmgtnls
pub(crate) fn append_quoted(arg: &OsStr, cmdline: &mut Vec<u16>) {
    if !arg.is_empty()
        && !arg.encode_wide().any(|c| {
            c == ' ' as u16
                || c == '\t' as u16
                || c == '\n' as u16
                || c == '\x0b' as u16
                || c == '\"' as u16
        })
    {
        cmdline.extend(arg.encode_wide());
        return;
    }
    cmdline.push('"' as u16);

    let arg: Vec<_> = arg.encode_wide().collect();
    let mut i = 0;
    while i < arg.len() {
        let mut num_backslashes = 0;
        while i < arg.len() && arg[i] == '\\' as u16 {
            i += 1;
            num_backslashes += 1;
        }

        if i == arg.len() {
            for _ in 0..num_backslashes * 2 {
                cmdline.push('\\' as u16);
            }
            break;
        } else if arg[i] == b'"' as u16 {
            for _ in 0..num_backslashes * 2 + 1 {
                cmdline.push('\\' as u16);
            }
            cmdline.push(arg[i]);
        } else {
            for _ in 0..num_backslashes {
                cmdline.push('\\' as u16);
            }
            cmdline.push(arg[i]);
        }
        i += 1;
    }
    cmdline.push('"' as u16);
}

#[cfg(test)]
#[path = "../../tests-rs/test_cmdbuilder.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests-rs/test_issue167_envblock.rs"]
mod tests_issue167_envblock;
