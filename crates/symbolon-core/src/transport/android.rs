//! Reaching Android devices through `adb` (spec §6, ADR-0017).
//!
//! [`AndroidTransport`] is what the Android source needs from a connection; [`SystemAdb`]
//! implements it by running the system `adb`, and tests substitute their own. Devices are
//! never authorized here: that is the user's choice on the phone.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

/// A device as `adb devices -l` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Device {
    /// The serial `adb -s` takes: a hardware serial over USB, `host:port` or an mDNS name over
    /// Wi-Fi.
    pub serial: String,
    /// Whether it can be used.
    pub state: DeviceState,
    /// Model name with underscores (`Pixel_10`), if reported.
    pub model: Option<String>,
    /// Product name, if reported.
    pub product: Option<String>,
}

/// Connection state of a device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceState {
    /// Connected and authorized.
    Device,
    /// Waiting for the user to accept the USB debugging prompt.
    Unauthorized,
    /// Connected but not responding.
    Offline,
    /// The host may not open the USB device; the text is `adb`'s explanation.
    NoPermissions(String),
    /// Booted into recovery.
    Recovery,
    /// In sideload mode.
    Sideload,
    /// Any other state, as `adb` names it.
    Other(String),
}

impl DeviceState {
    /// What the user can do about this state, if anything.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Device => None,
            Self::Unauthorized => Some(
                "accept the 'Allow USB debugging' prompt on the phone (unlock it to see it)"
                    .to_owned(),
            ),
            Self::Offline => {
                Some("reconnect the cable, or restart adb with `adb kill-server`".to_owned())
            }
            Self::NoPermissions(why) => Some(format!(
                "this user may not open the USB device ({why}); install Android udev rules or \
                 add yourself to the plugdev group"
            )),
            Self::Recovery | Self::Sideload => {
                Some("boot the phone normally to scan it".to_owned())
            }
            Self::Other(_) => Some("wait until adb reports the device as ready".to_owned()),
        }
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device => f.write_str("device"),
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::Offline => f.write_str("offline"),
            Self::NoPermissions(_) => f.write_str("no permissions"),
            Self::Recovery => f.write_str("recovery"),
            Self::Sideload => f.write_str("sideload"),
            Self::Other(state) => f.write_str(state),
        }
    }
}

/// Parses the output of `adb devices -l`. Lines it does not understand (the header, daemon
/// messages) are skipped.
///
/// # Examples
///
/// ```
/// use symbolon_core::transport::android::{DeviceState, parse_devices};
///
/// let devices = parse_devices(
///     "List of devices attached\n\
///      39151FDJH000LN  device usb:1-3 product:frankel model:Pixel_10 transport_id:4\n\
///      R58M12345       unauthorized usb:1-1 transport_id:2\n",
/// );
/// assert_eq!(devices[0].model.as_deref(), Some("Pixel_10"));
/// assert_eq!(devices[1].state, DeviceState::Unauthorized);
/// ```
pub fn parse_devices(text: &str) -> Vec<Device> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Device> {
    let line = line.trim_end();
    if line.is_empty() || line.starts_with('*') || line.starts_with("List of devices") {
        return None;
    }
    let (serial, rest) = line.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (state, details) = if let Some(after) = rest.strip_prefix("no permissions") {
        let why = after
            .trim_start()
            .strip_prefix('(')
            .and_then(|s| s.split_once(')'))
            .map_or(String::new(), |(why, _)| why.to_owned());
        (DeviceState::NoPermissions(why), after)
    } else {
        let (state, details) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let state = match state {
            "device" => DeviceState::Device,
            "unauthorized" => DeviceState::Unauthorized,
            "offline" => DeviceState::Offline,
            "recovery" => DeviceState::Recovery,
            "sideload" => DeviceState::Sideload,
            other => DeviceState::Other(other.to_owned()),
        };
        (state, details)
    };
    let field = |key: &str| {
        details
            .split_whitespace()
            .find_map(|token| token.strip_prefix(key)?.strip_prefix(':'))
            .map(str::to_owned)
    };
    Some(Device {
        serial: serial.to_owned(),
        state,
        model: field("model"),
        product: field("product"),
    })
}

/// Errors from talking to `adb` or a device.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AdbError {
    /// `adb` itself could not be started.
    #[error(
        "cannot run {}: {source}; install Android platform-tools, or set SYMBOLON_ADB to its path",
        program.display()
    )]
    NotFound {
        /// The program that was tried.
        program: PathBuf,
        /// Why it could not be started.
        source: io::Error,
    },
    /// An `adb` command ran and failed.
    #[error("`adb {command}` failed{}: {message}", status.map_or_else(String::new, |s| format!(" with status {s}")))]
    Failed {
        /// The arguments, for the message.
        command: String,
        /// Exit status, if the process exited normally.
        status: Option<i32>,
        /// What it printed on stderr (or stdout when stderr was empty).
        message: String,
    },
    /// Reading from or writing to `adb` failed.
    #[error("talking to adb: {0}")]
    Io(#[from] io::Error),
}

/// The output of a finished shell command on the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    /// The command's exit status on the device.
    pub status: i32,
    /// What it wrote to stdout.
    pub stdout: Vec<u8>,
    /// What it wrote to stderr.
    pub stderr: String,
}

/// The standard output of a command running on the device, read while it runs.
pub trait ExecStream: Read + Send {
    /// Waits for the command and reports a failure of the transport itself. The command's own
    /// success is its output's business (the agent ends with a `done` line).
    ///
    /// # Errors
    ///
    /// [`AdbError::Failed`] if `adb` reported an error.
    fn finish(self: Box<Self>) -> Result<(), AdbError>;
}

/// A connection to Android devices.
pub trait AndroidTransport: Send + Sync {
    /// Every device `adb` knows, in any state.
    ///
    /// # Errors
    ///
    /// `adb` cannot be run.
    fn devices(&self) -> Result<Vec<Device>, AdbError>;

    /// Copies a local file to `remote` on the device.
    ///
    /// # Errors
    ///
    /// The copy failed.
    fn push(&self, serial: &str, local: &Path, remote: &str) -> Result<(), AdbError>;

    /// Runs `argv` on the device (each element one argument, quoted for the device shell) and
    /// streams its standard output, unaltered (`adb exec-out`).
    ///
    /// # Errors
    ///
    /// The command could not be started.
    fn exec(&self, serial: &str, argv: &[String]) -> Result<Box<dyn ExecStream>, AdbError>;

    /// Runs a shell command line on the device and waits for it (`adb shell`).
    ///
    /// # Errors
    ///
    /// `adb` could not run the command. A command that runs and fails is a [`ShellOutput`]
    /// with a non-zero status.
    fn shell(&self, serial: &str, command: &str) -> Result<ShellOutput, AdbError>;
}

/// Quotes `arg` for a POSIX shell (the device's `sh`): single quotes, with `'` written as
/// `'\''`.
///
/// ```
/// use symbolon_core::transport::android::shell_quote;
///
/// assert_eq!(shell_quote("/sdcard/it's here"), r"'/sdcard/it'\''s here'");
/// ```
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// The system `adb`: `SYMBOLON_ADB` if set, else `adb` on `PATH`.
#[derive(Debug, Clone)]
pub struct SystemAdb {
    program: PathBuf,
}

impl Default for SystemAdb {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemAdb {
    /// `adb` from `SYMBOLON_ADB` or `PATH`.
    pub fn new() -> Self {
        let program =
            std::env::var_os("SYMBOLON_ADB").map_or_else(|| PathBuf::from("adb"), PathBuf::from);
        Self { program }
    }

    /// A specific `adb` executable.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    fn command(&self, args: &[OsString]) -> Command {
        let mut command = Command::new(&self.program);
        command.args(args);
        command
    }

    fn spawn(&self, args: &[OsString], stdout: Stdio) -> Result<Child, AdbError> {
        self.command(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| AdbError::NotFound {
                program: self.program.clone(),
                source,
            })
    }

    /// Runs `adb args…` to completion.
    fn run(&self, args: &[OsString]) -> Result<std::process::Output, AdbError> {
        Ok(self.spawn(args, Stdio::piped())?.wait_with_output()?)
    }
}

fn describe(args: &[OsString]) -> String {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn failed(args: &[OsString], output: &std::process::Output) -> AdbError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let message = if stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    } else {
        stderr
    };
    AdbError::Failed {
        command: describe(args),
        status: output.status.code(),
        message,
    }
}

fn with_serial(serial: &str, rest: &[&str]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-s".into(), serial.into()];
    args.extend(rest.iter().map(OsString::from));
    args
}

impl AndroidTransport for SystemAdb {
    fn devices(&self) -> Result<Vec<Device>, AdbError> {
        let args = vec![OsString::from("devices"), OsString::from("-l")];
        let output = self.run(&args)?;
        if !output.status.success() {
            return Err(failed(&args, &output));
        }
        Ok(parse_devices(&String::from_utf8_lossy(&output.stdout)))
    }

    fn push(&self, serial: &str, local: &Path, remote: &str) -> Result<(), AdbError> {
        let mut args = with_serial(serial, &["push"]);
        args.push(local.as_os_str().to_owned());
        args.push(remote.into());
        let output = self.run(&args)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(failed(&args, &output))
        }
    }

    fn exec(&self, serial: &str, argv: &[String]) -> Result<Box<dyn ExecStream>, AdbError> {
        let line = argv
            .iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ");
        let adb_args = with_serial(serial, &["exec-out", &line]);
        let mut child = self.spawn(&adb_args, Stdio::piped())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdbError::Io(io::Error::other("adb exec-out has no standard output")))?;
        Ok(Box::new(Running {
            child,
            stdout,
            args: adb_args,
        }))
    }

    fn shell(&self, serial: &str, command: &str) -> Result<ShellOutput, AdbError> {
        let args = with_serial(serial, &["shell", command]);
        let output = self.run(&args)?;
        // `adb shell` exits with the remote status; 255 with nothing on stdout means adb itself
        // failed (no device, connection lost).
        let status = output.status.code().unwrap_or(-1);
        if status == 255 && output.stdout.is_empty() {
            return Err(failed(&args, &output));
        }
        Ok(ShellOutput {
            status,
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// A running `adb exec-out`.
struct Running {
    child: Child,
    stdout: ChildStdout,
    args: Vec<OsString>,
}

impl Read for Running {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdout.read(buf)
    }
}

impl ExecStream for Running {
    fn finish(mut self: Box<Self>) -> Result<(), AdbError> {
        // Drain what the caller did not read, so adb is not blocked writing to a full pipe.
        io::copy(&mut self.stdout, &mut io::sink())?;
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            pipe.read_to_string(&mut stderr)?;
        }
        let status = self.child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(AdbError::Failed {
                command: describe(&self.args),
                status: status.code(),
                message: stderr.trim().to_owned(),
            })
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // A stream dropped without `finish` (an error or a cancel) must not leave adb running.
        if matches!(self.child.try_wait(), Ok(None)) {
            self.child.kill().ok();
            self.child.wait().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
* daemon not running; starting now at tcp:5037
* daemon started successfully
List of devices attached
39151FDJH000LN         device usb:1-3 product:frankel model:Pixel_10 device:frankel transport_id:4
192.168.1.20:5555      device product:a54x model:SM_A546B device:a54x transport_id:5
adb-39151FDJH000LN-AbCdEf._adb-tls-connect._tcp device product:frankel model:Pixel_10 device:frankel transport_id:7
R58M12345              unauthorized usb:1-1 transport_id:2
emulator-5554          offline transport_id:3
0123456789ABCDEF       no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html] usb:1-2 transport_id:6
ZY22                   recovery usb:1-4 transport_id:8
ZY23                   authorizing usb:1-5 transport_id:9

";

    #[test]
    fn every_state_of_adb_devices_is_parsed() {
        let devices = parse_devices(SAMPLE);
        let summary: Vec<(&str, String, Option<&str>)> = devices
            .iter()
            .map(|d| (d.serial.as_str(), d.state.to_string(), d.model.as_deref()))
            .collect();
        assert_eq!(
            summary,
            [
                ("39151FDJH000LN", "device".into(), Some("Pixel_10")),
                ("192.168.1.20:5555", "device".into(), Some("SM_A546B")),
                (
                    "adb-39151FDJH000LN-AbCdEf._adb-tls-connect._tcp",
                    "device".into(),
                    Some("Pixel_10")
                ),
                ("R58M12345", "unauthorized".into(), None),
                ("emulator-5554", "offline".into(), None),
                ("0123456789ABCDEF", "no permissions".into(), None),
                ("ZY22", "recovery".into(), None),
                ("ZY23", "authorizing".into(), None),
            ]
        );
        assert_eq!(devices[0].product.as_deref(), Some("frankel"));
        assert_eq!(
            devices[5].state,
            DeviceState::NoPermissions(
                "missing udev rules? user is in the plugdev group".to_owned()
            )
        );
        assert!(devices.iter().skip(3).all(|d| d.state.hint().is_some()));
        assert_eq!(devices[0].state.hint(), None);
    }

    #[test]
    fn arguments_are_quoted_for_the_device_shell() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b$(x)`y`"), "'a b$(x)`y`'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn a_missing_adb_says_how_to_get_one() {
        let adb = SystemAdb::with_program("/nonexistent/adb");
        let error = adb.devices().unwrap_err();
        assert!(matches!(error, AdbError::NotFound { .. }));
        assert!(error.to_string().contains("SYMBOLON_ADB"), "{error}");
    }

    /// A fake `adb` that answers `devices -l` and echoes everything else, to check the
    /// command lines `SystemAdb` builds.
    #[cfg(unix)]
    mod fake {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        fn fake_adb(dir: &Path) -> SystemAdb {
            let script = dir.join("adb");
            std::fs::write(
                &script,
                "#!/bin/sh\n\
                 if [ \"$1\" = devices ]; then printf 'List of devices attached\\nS1 device model:X\\n'; exit 0; fi\n\
                 case \"$*\" in *fail*) echo 'error: device not found' >&2; exit 1;; esac\n\
                 if [ \"$3\" = shell ]; then printf '%s' \"$4\"; exit 3; fi\n\
                 for a in \"$@\"; do printf '[%s]' \"$a\"; done\n",
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            SystemAdb::with_program(script)
        }

        #[test]
        fn commands_reach_adb_as_expected() {
            let dir = tempfile::tempdir().unwrap();
            let adb = fake_adb(dir.path());
            let devices = adb.devices().unwrap();
            assert_eq!(devices.len(), 1);
            assert_eq!(devices[0].model.as_deref(), Some("X"));

            let mut out = String::new();
            let mut stream = adb
                .exec("S1", &["/data/local/tmp/agent".into(), "it's".into()])
                .unwrap();
            stream.read_to_string(&mut out).unwrap();
            stream.finish().unwrap();
            assert_eq!(
                out,
                r"[-s][S1][exec-out]['/data/local/tmp/agent' 'it'\''s']"
            );

            let shell = adb.shell("S1", "echo hi").unwrap();
            assert_eq!(
                (shell.status, shell.stdout.as_slice()),
                (3, &b"echo hi"[..])
            );

            let error = adb.push("S1", Path::new("/x"), "fail").unwrap_err();
            assert!(
                matches!(&error, AdbError::Failed { status: Some(1), message, .. } if message == "error: device not found"),
                "{error:?}"
            );
        }
    }
}
