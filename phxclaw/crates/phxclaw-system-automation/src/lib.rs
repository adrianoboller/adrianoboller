use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    Direct,
    Cmd,
    PowerShell,
    Sh,
    Bash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub uuid: Uuid,
    pub shell: ShellKind,
    pub program_or_script: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub stdin: Option<String>,
    pub timeout_ms: u64,
}

impl CommandRequest {
    pub fn direct(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            uuid: new_uuid_v7(),
            shell: ShellKind::Direct,
            program_or_script: program.into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
            stdin: None,
            timeout_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub request_uuid: Uuid,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub elapsed_ms: u128,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub uuid: Uuid,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
}

impl LaunchRequest {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            uuid: new_uuid_v7(),
            program: program.into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    pub request_uuid: Uuid,
    pub pid: u32,
    pub launched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub enabled: bool,
    pub allow_shells: bool,
    pub max_timeout_ms: u64,
    pub denied_programs: Vec<String>,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_shells: false,
            max_timeout_ms: 300_000,
            denied_programs: vec![],
        }
    }
}

#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("system command execution is disabled by policy")]
    Disabled,
    #[error("shell execution is disabled by policy")]
    ShellDisabled,
    #[error("program denied by policy: {0}")]
    DeniedProgram(String),
    #[error("requested timeout exceeds policy")]
    TimeoutPolicy,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported shell on this platform: {0:?}")]
    UnsupportedShell(ShellKind),
}

pub struct ShellExecutor {
    policy: ExecutionPolicy,
}

impl ShellExecutor {
    pub fn new(policy: ExecutionPolicy) -> Self { Self { policy } }

    pub fn launch(&self, request: &LaunchRequest) -> Result<LaunchResult, AutomationError> {
        if !self.policy.enabled { return Err(AutomationError::Disabled); }
        let program_key = request.program.trim().to_ascii_lowercase();
        if self.policy.denied_programs.iter().any(|p| p.eq_ignore_ascii_case(&program_key)) {
            return Err(AutomationError::DeniedProgram(request.program.clone()));
        }
        let mut command = Command::new(&request.program);
        command.args(&request.args);
        if let Some(cwd) = &request.cwd { command.current_dir(cwd); }
        command.envs(&request.env);
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let child = command.spawn()?;
        Ok(LaunchResult {
            request_uuid: request.uuid,
            pid: child.id(),
            launched_at: Utc::now(),
        })
    }

    pub fn execute(&self, request: &CommandRequest) -> Result<CommandResult, AutomationError> {
        if !self.policy.enabled { return Err(AutomationError::Disabled); }
        if !matches!(request.shell, ShellKind::Direct) && !self.policy.allow_shells {
            return Err(AutomationError::ShellDisabled);
        }
        if request.timeout_ms == 0 || request.timeout_ms > self.policy.max_timeout_ms {
            return Err(AutomationError::TimeoutPolicy);
        }
        let program_key = request.program_or_script.trim().to_ascii_lowercase();
        if self.policy.denied_programs.iter().any(|p| p.eq_ignore_ascii_case(&program_key)) {
            return Err(AutomationError::DeniedProgram(request.program_or_script.clone()));
        }

        let mut command = command_for(request)?;
        if let Some(cwd) = &request.cwd { command.current_dir(cwd); }
        command.envs(&request.env);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if request.stdin.is_some() { command.stdin(Stdio::piped()); }

        let started = Instant::now();
        let mut child = command.spawn()?;
        if let (Some(input), Some(mut stdin)) = (&request.stdin, child.stdin.take()) {
            stdin.write_all(input.as_bytes())?;
        }

        let timeout = Duration::from_millis(request.timeout_ms);
        let mut timed_out = false;
        loop {
            if child.try_wait()?.is_some() { break; }
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = child.kill();
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let output = child.wait_with_output()?;
        Ok(CommandResult {
            request_uuid: request.uuid,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            timed_out,
            elapsed_ms: started.elapsed().as_millis(),
            finished_at: Utc::now(),
        })
    }
}

fn command_for(request: &CommandRequest) -> Result<Command, AutomationError> {
    let mut command = match request.shell {
        ShellKind::Direct => {
            let mut c = Command::new(&request.program_or_script);
            c.args(&request.args);
            return Ok(c);
        }
        ShellKind::Cmd if cfg!(windows) => {
            let mut c = Command::new("cmd.exe"); c.args(["/D", "/S", "/C", &request.program_or_script]); c
        }
        ShellKind::PowerShell if cfg!(windows) => {
            let mut c = Command::new("powershell.exe"); c.args(["-NoProfile", "-NonInteractive", "-Command", &request.program_or_script]); c
        }
        ShellKind::Sh if cfg!(unix) => {
            let mut c = Command::new("/bin/sh"); c.args(["-lc", &request.program_or_script]); c
        }
        ShellKind::Bash if cfg!(unix) => {
            let mut c = Command::new("bash"); c.args(["-lc", &request.program_or_script]); c
        }
        other => return Err(AutomationError::UnsupportedShell(other)),
    };
    command.args(&request.args);
    Ok(command)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InputAction {
    MoveMouse { x: i32, y: i32 },
    MouseButton { button: String, state: String },
    Scroll { dx: i32, dy: i32 },
    Key { key: String, state: String },
    Text { text: String },
    Hotkey { keys: Vec<String> },
}

/// OS-specific input providers (Enigo on Windows/macOS/Linux is the planned default).
pub trait InputProvider {
    fn apply(&mut self, action: &InputAction) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequest {
    pub output: PathBuf,
    pub format: String,
    pub target: String,
    pub duration_ms: Option<u64>,
    pub fps: Option<u32>,
}

/// Generates an ffmpeg command for desktop/window capture. Device syntax is intentionally
/// platform-specific and must be supplied by the signed capture plugin after permission checks.
pub fn ffmpeg_record_command(input_args: &[String], output: &str) -> CommandRequest {
    let mut args = vec!["-y".to_string()];
    args.extend_from_slice(input_args);
    args.push(output.to_string());
    CommandRequest::direct("ffmpeg", args)
}


#[cfg(feature = "desktop-input")]
pub struct EnigoInputProvider {
    inner: enigo::Enigo,
}

#[cfg(feature = "desktop-input")]
impl EnigoInputProvider {
    pub fn new() -> Result<Self, String> {
        use enigo::Settings;
        let inner = enigo::Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "desktop-input")]
impl InputProvider for EnigoInputProvider {
    fn apply(&mut self, action: &InputAction) -> Result<(), String> {
        use enigo::{Axis, Button, Coordinate, Direction, Key, Keyboard, Mouse};
        fn direction(raw: &str) -> Result<Direction, String> {
            match raw.to_ascii_lowercase().as_str() {
                "press" | "down" => Ok(Direction::Press),
                "release" | "up" => Ok(Direction::Release),
                "click" | "tap" => Ok(Direction::Click),
                other => Err(format!("unknown direction: {other}")),
            }
        }
        fn button(raw: &str) -> Result<Button, String> {
            match raw.to_ascii_lowercase().as_str() {
                "left" | "1" => Ok(Button::Left),
                "middle" | "2" => Ok(Button::Middle),
                "right" | "3" => Ok(Button::Right),
                "back" => Ok(Button::Back),
                "forward" => Ok(Button::Forward),
                other => Err(format!("unknown mouse button: {other}")),
            }
        }
        fn key(raw: &str) -> Result<Key, String> {
            let lower = raw.to_ascii_lowercase();
            let named = match lower.as_str() {
                "ctrl" | "control" => Some(Key::Control),
                "alt" => Some(Key::Alt),
                "shift" => Some(Key::Shift),
                "meta" | "super" | "win" | "windows" | "command" => Some(Key::Meta),
                "enter" | "return" => Some(Key::Return),
                "esc" | "escape" => Some(Key::Escape),
                "tab" => Some(Key::Tab),
                "space" => Some(Key::Space),
                "backspace" => Some(Key::Backspace),
                "delete" | "del" => Some(Key::Delete),
                "home" => Some(Key::Home),
                "end" => Some(Key::End),
                "pageup" => Some(Key::PageUp),
                "pagedown" => Some(Key::PageDown),
                "up" => Some(Key::UpArrow),
                "down" => Some(Key::DownArrow),
                "left" => Some(Key::LeftArrow),
                "right" => Some(Key::RightArrow),
                "f1" => Some(Key::F1), "f2" => Some(Key::F2), "f3" => Some(Key::F3),
                "f4" => Some(Key::F4), "f5" => Some(Key::F5), "f6" => Some(Key::F6),
                "f7" => Some(Key::F7), "f8" => Some(Key::F8), "f9" => Some(Key::F9),
                "f10" => Some(Key::F10), "f11" => Some(Key::F11), "f12" => Some(Key::F12),
                _ => None,
            };
            if let Some(k) = named { return Ok(k); }
            let mut chars = raw.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(Key::Unicode(c)),
                _ => Err(format!("unknown key: {raw}")),
            }
        }

        match action {
            InputAction::MoveMouse { x, y } => self.inner.move_mouse(*x, *y, Coordinate::Abs),
            InputAction::MouseButton { button: b, state } => self.inner.button(button(b)?, direction(state)?),
            InputAction::Scroll { dx, dy } => {
                if *dx != 0 { self.inner.scroll(*dx, Axis::Horizontal).map_err(|e| e.to_string())?; }
                if *dy != 0 { self.inner.scroll(*dy, Axis::Vertical).map_err(|e| e.to_string())?; }
                return Ok(());
            }
            InputAction::Key { key: k, state } => self.inner.key(key(k)?, direction(state)?),
            InputAction::Text { text } => self.inner.text(text),
            InputAction::Hotkey { keys } => {
                let parsed = keys.iter().map(|k| key(k)).collect::<Result<Vec<_>, _>>()?;
                for k in &parsed { self.inner.key(*k, Direction::Press).map_err(|e| e.to_string())?; }
                for k in parsed.iter().rev() { self.inner.key(*k, Direction::Release).map_err(|e| e.to_string())?; }
                return Ok(());
            }
        }.map_err(|e| e.to_string())
    }
}

#[cfg(feature = "screen-capture")]
pub fn capture_primary_monitor(output: &std::path::Path) -> Result<(), AutomationError> {
    let monitors = xcap::Monitor::all().map_err(|e| AutomationError::Io(std::io::Error::other(e.to_string())))?;
    let monitor = monitors.into_iter().find(|m| m.is_primary().unwrap_or(false))
        .ok_or_else(|| AutomationError::Io(std::io::Error::other("no primary monitor")))?;
    let image = monitor.capture_image().map_err(|e| AutomationError::Io(std::io::Error::other(e.to_string())))?;
    image.save(output).map_err(|e| AutomationError::Io(std::io::Error::other(e.to_string())))?;
    Ok(())
}
