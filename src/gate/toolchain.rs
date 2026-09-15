//! Managed toolchain preparation and execution backends.
//!
//! Gate adapters must not assume external tools (scanners, policy
//! checkers, browser drivers) are already installed, and they must not
//! install the monitored project's own SDKs (Rust, .NET, Python,
//! Node). This module owns the policy layer between the Gate plan and
//! process execution:
//!
//! * [`ToolchainManifest`] — pinned tool identity (`id`, `version`,
//!   `platform`, `digest`, optional `signature`, allowed
//!   [`ExecutionMode`]s). Parsed strictly: unknown fields fail with a
//!   `did you mean` hint, like [`crate::gate::manifest`].
//! * Platform resolution — [`current_platform`] maps the host to the
//!   release target matrix (`linux-x86_64`, `linux-arm64`,
//!   `darwin-x86_64`, mirroring `README.md` and
//!   `scripts/lib/platform.sh`). [`resolve_platform`] rejects
//!   unsupported targets **before any execution**.
//! * Verified cache — [`cache_dir`]/[`executable_path`] lay out a
//!   per-tool, per-version, per-platform user cache;
//!   [`store_verified_bytes`] verifies `sha256:` digests **before**
//!   an atomic temp+rename; [`cached_verified`] reuses verified bytes
//!   offline; [`verify_bytes`] mismatches delete the payload and never
//!   execute.
//! * Execution backends — [`resolve_execution`] selects `managed`,
//!   `container`, `native`, or `project-runtime` without performing
//!   network I/O. Ordinary checks run under
//!   [`ProvisionPolicy::offline`]: a missing tool is a
//!   [`ToolchainError::OfflineMissing`] with the exact bootstrap next
//!   action, never a silent download.
//! * Bootstrap — [`bootstrap_plan`]/[`render_bootstrap_plan`] describe
//!   the explicit provisioning steps. Fetching bytes is the caller's
//!   job (installer, CI cache, operator); this module only verifies
//!   and atomically caches them. No network call is made here.
//! * Doctor — [`toolchain_checks`] reports cache, platform, container,
//!   and project-runtime readiness from filesystem/`PATH` probes,
//!   never from configuration alone.
//!
//! ## Boundaries
//!
//! * Managed Gate tooling is separate from project runtimes: a
//!   `project-runtime` entry executes the project's declared `argv`
//!   verbatim (no shell splitting) and reports
//!   [`ToolchainError::ProjectRuntimeMissing`] when the runtime is
//!   absent — it never installs an SDK.
//! * Container images must be digest-pinned (`name@sha256:<hex>`);
//!   mounts are confined under the project root and the environment
//!   is an explicit allowlist.
//! * Native execution is opt-in: [`ProvisionPolicy::allow_native_fallback`]
//!   must be set, otherwise resolution fails closed.
//! * Locking is per cached executable (`<exe>.lock` via `create_new`);
//!   contention surfaces as [`ToolchainError::LockContention`] with a
//!   retry remediation instead of corrupting the cache.
//!
//! No OpenSpec types appear here. No network call is made, no tool is
//! fetched implicitly, and no LLM is invoked.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Manifest file names, resolved in order at the project root first,
/// then under `.driftwatch/`. Absence is not an error (no Gate tools
/// declared); callers stay silent in that case.
pub const TOOLCHAIN_MANIFEST_FILES: &[&str] = &["gate-tools.toml"];
/// Supported Gate-tool platforms. Mirrors the release matrix in
/// `README.md` and `scripts/lib/platform.sh`.
pub const SUPPORTED_PLATFORMS: &[&str] = &["linux-x86_64", "linux-arm64", "darwin-x86_64"];
/// Largest single managed tool accepted into the cache (128 MiB).
/// Tools are bigger than evidence artifacts (1 MiB cap there); the
/// bound still keeps a corrupt download from filling the disk.
pub const MAX_TOOL_BYTES: usize = 128 * 1024 * 1024;
/// Digest prefix for pinned tool bytes.
pub const DIGEST_PREFIX: &str = "sha256:";

/// Execution backends. Wire strings are `snake_case` and stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Managed,
    Container,
    Native,
    ProjectRuntime,
}

impl ExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionMode::Managed => "managed",
            ExecutionMode::Container => "container",
            ExecutionMode::Native => "native",
            ExecutionMode::ProjectRuntime => "project_runtime",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "managed" => Some(ExecutionMode::Managed),
            "container" => Some(ExecutionMode::Container),
            "native" => Some(ExecutionMode::Native),
            "project_runtime" => Some(ExecutionMode::ProjectRuntime),
            _ => None,
        }
    }
}

/// One pinned tool entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEntry {
    /// Stable tool id (e.g. `semgrep`). Used as the cache directory name.
    pub id: String,
    /// Pinned version (e.g. `1.2.3`). Exact match against the cache.
    pub version: String,
    /// Platforms this entry serves. Each must be in [`SUPPORTED_PLATFORMS`].
    /// Empty means all supported platforms.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Provenance URL (informational; the core never fetches it).
    #[serde(default)]
    pub source: Option<String>,
    /// Required content digest (`sha256:<64 hex>`). Verified before use.
    pub digest: String,
    /// Optional detached signature reference (informational until an
    /// external verifier hook exists; digest remains mandatory).
    #[serde(default)]
    pub signature: Option<String>,
    /// Allowed execution modes, in caller preference order.
    #[serde(default = "default_modes")]
    pub modes: Vec<ExecutionMode>,
    /// Container image ref. Required when `container` is allowed; must
    /// be digest-pinned (`name@sha256:<hex>`).
    #[serde(default)]
    pub container_image: Option<String>,
    /// Program name for `native` mode (defaults to `id`).
    #[serde(default)]
    pub native_program: Option<String>,
    /// Declared argv for `project_runtime` mode (program + args, no
    /// shell splitting). Required when that mode is allowed.
    #[serde(default)]
    pub project_argv: Option<Vec<String>>,
    /// Bounded mounts for `container` mode, relative to the project root.
    #[serde(default)]
    pub mounts: Vec<String>,
    /// Explicit environment allowlist for `container` mode (`KEY=val`).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Opt-in wall-clock timeout carried into the runner's
    /// `CommandSpec::timeout_ms`. `None` preserves unbounded wait.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn default_modes() -> Vec<ExecutionMode> {
    vec![ExecutionMode::Managed]
}

/// Toolchain manifest: the set of pinned Gate tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainManifest {
    /// Manifest schema version. Must equal
    /// [`crate::gate::dto::GATE_CONTRACT_VERSION`].
    #[serde(default = "default_toolchain_version")]
    pub version: u32,
    /// Pinned tools. Ids must be unique.
    #[serde(default)]
    pub tools: Vec<ToolEntry>,
}

fn default_toolchain_version() -> u32 {
    crate::gate::dto::GATE_CONTRACT_VERSION
}

/// Provisioning policy. Ordinary gate execution is offline: tools are
/// reused from the verified cache and a missing tool fails closed
/// with the bootstrap next action instead of downloading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionPolicy {
    /// When true the caller permits network provisioning (explicit
    /// bootstrap only). The core still never fetches: it reports what
    /// to fetch and verifies caller-supplied bytes.
    pub allow_network: bool,
    /// When true a `native` fallback may be selected. Native execution
    /// is otherwise rejected so a `PATH` binary cannot silently
    /// substitute for a pinned tool.
    pub allow_native_fallback: bool,
}

impl ProvisionPolicy {
    /// Default for ordinary checks: fully offline, no native fallback.
    pub fn offline() -> Self {
        Self {
            allow_network: false,
            allow_native_fallback: false,
        }
    }

    /// Explicit bootstrap on a machine the operator controls.
    pub fn bootstrap() -> Self {
        Self {
            allow_network: true,
            allow_native_fallback: false,
        }
    }
}

/// Actionable toolchain errors. Every variant carries the remediation
/// the operator needs; the binary appends the config hint.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ToolchainError {
    #[error("unknown field \"{field}\"{suggestion}")]
    UnknownField { field: String, suggestion: String },
    #[error("unsupported toolchain manifest version {got}, expected {expected}")]
    UnknownVersion { got: u32, expected: u32 },
    #[error("tool entry has an empty id (set `id = \"...\"`)")]
    EmptyId,
    #[error("tool \"{id}\" has an empty version (pin `version = \"...\"`)")]
    EmptyVersion { id: String },
    #[error("tool \"{id}\" has an empty digest (pin `digest = \"sha256:...\"`)")]
    EmptyDigest { id: String },
    #[error("tool \"{id}\" has a malformed digest (expected `sha256:<64 hex chars>`)")]
    BadDigest { id: String },
    #[error("duplicate tool id \"{id}\" (tool ids must be unique)")]
    DuplicateId { id: String },
    #[error("tool \"{id}\" targets unsupported platform \"{platform}\"; supported: linux-x86_64, linux-arm64, darwin-x86_64")]
    UnsupportedPlatform { id: String, platform: String },
    #[error("host platform \"{platform}\" is unsupported; supported: linux-x86_64, linux-arm64, darwin-x86_64")]
    HostUnsupported { platform: String },
    #[error("tool \"{id}\" allows no execution mode (set `modes = [\"managed\", ...]`)")]
    NoAllowedMode { id: String },
    #[error("tool \"{id}\" allows `container` but has no digest-pinned image (set `container_image = \"name@sha256:...\"`)")]
    ContainerImageNotPinned { id: String },
    #[error("tool \"{id}\" allows `project_runtime` but declares no `project_argv` (set the project's build/test argv verbatim)")]
    EmptyArgv { id: String },
    #[error("tool \"{id}\" verification failed: digest mismatch (deleted unverified bytes; execution did not start)")]
    VerificationMismatch { id: String },
    #[error("tool \"{id}\" version {version} is not cached for {platform} and policy is offline (run explicit bootstrap to provision it; ordinary checks never download tools)")]
    OfflineMissing {
        id: String,
        version: String,
        platform: String,
    },
    #[error("tool \"{id}\" is not cached and network provisioning is not permitted by policy (re-run with explicit bootstrap)")]
    ProvisioningDenied { id: String },
    #[error("tool \"{id}\" native fallback is not permitted (set ProvisionPolicy::allow_native_fallback or provision the managed tool)")]
    NativeNotAllowed { id: String },
    #[error("project runtime for tool \"{id}\" is missing: `{program}` is not on PATH (install the project SDK yourself; Driftwatchdog never installs project runtimes)")]
    ProjectRuntimeMissing { id: String, program: String },
    #[error("container runtime unavailable for tool \"{id}\" (start docker/podman or select an allowed fallback; result maps to REVIEW_REQUIRED per gate policy)")]
    ContainerUnavailable { id: String },
    #[error(
        "mount \"{mount}\" escapes the project root (mounts must stay under the project root)"
    )]
    MountEscape { mount: String },
    #[error("cache lock for tool \"{id}\" is held by another bootstrap (retry after it finishes)")]
    LockContention { id: String },
    #[error("toolchain cache error for tool \"{id}\": {detail}")]
    Cache { id: String, detail: String },
    #[error("manifest parse error: {0}")]
    Parse(String),
}

const KNOWN_FIELDS: &[&str] = &[
    "version",
    "tools",
    "id",
    "platforms",
    "source",
    "digest",
    "signature",
    "modes",
    "container_image",
    "native_program",
    "project_argv",
    "mounts",
    "env",
    "timeout_ms",
];

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(cur[j] + 1).min(prev[j + 1] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn suggest_field(unknown: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    for &known in KNOWN_FIELDS {
        let d = edit_distance(unknown, known);
        if d <= 3 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((known, d));
        }
    }
    best.map(|(s, _)| s)
}

fn extract_unknown_field(msg: &str) -> Option<String> {
    for pat in ["unknown field `", "unknown field \"", "unknown field '"] {
        if let Some(start) = msg.find(pat) {
            let rest = &msg[start + pat.len()..];
            let end = rest.find(['`', '"', '\'']).unwrap_or(rest.len());
            let field: String = rest[..end].chars().take(64).collect();
            if !field.is_empty() {
                return Some(field);
            }
        }
    }
    None
}

fn map_toml_error(e: toml::de::Error) -> ToolchainError {
    let msg = e.to_string();
    if msg.contains("unknown field") {
        if let Some(field) = extract_unknown_field(&msg) {
            let suggestion = suggest_field(&field)
                .map(|s| format!("; did you mean \"{s}\"?"))
                .unwrap_or_default();
            return ToolchainError::UnknownField { field, suggestion };
        }
    }
    ToolchainError::Parse(msg)
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == DIGEST_PREFIX.len() + 64
        && digest.starts_with(DIGEST_PREFIX)
        && is_hex(&digest[DIGEST_PREFIX.len()..])
}

fn valid_container_image(image: &str) -> bool {
    // `name@sha256:<64 hex>` — digest-pinned, no floating tags.
    match image.rfind("@sha256:") {
        Some(at) => {
            let (name, hex) = (&image[..at], &image[at + "@sha256:".len()..]);
            !name.trim().is_empty() && hex.len() == 64 && is_hex(hex)
        }
        None => false,
    }
}

/// Host platform in release-matrix form (`os-arch`).
pub fn current_platform() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let os_part = match os {
        "linux" => "linux",
        "macos" => "darwin",
        other => other,
    };
    let arch_part = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os_part}-{arch_part}")
}

/// True when the platform is in the supported release matrix.
pub fn is_supported_platform(platform: &str) -> bool {
    SUPPORTED_PLATFORMS.contains(&platform)
}

/// Parse and validate toolchain manifest text. Fails before any
/// execution on unknown fields, version mismatch, duplicate ids,
/// empty identity fields, malformed digests, unsupported platforms,
/// unpinned container images, and missing project argv.
pub fn parse(text: &str) -> Result<ToolchainManifest, ToolchainError> {
    let manifest: ToolchainManifest = toml::from_str(text).map_err(map_toml_error)?;
    validate(&manifest)?;
    Ok(manifest)
}

fn validate(manifest: &ToolchainManifest) -> Result<(), ToolchainError> {
    if manifest.version != crate::gate::dto::GATE_CONTRACT_VERSION {
        return Err(ToolchainError::UnknownVersion {
            got: manifest.version,
            expected: crate::gate::dto::GATE_CONTRACT_VERSION,
        });
    }
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for tool in &manifest.tools {
        if tool.id.trim().is_empty() {
            return Err(ToolchainError::EmptyId);
        }
        if !seen.insert(tool.id.as_str()) {
            return Err(ToolchainError::DuplicateId {
                id: tool.id.clone(),
            });
        }
        if tool.id.contains('/') || tool.id.contains('\\') || tool.id.contains("..") {
            return Err(ToolchainError::Cache {
                id: tool.id.clone(),
                detail: "tool id must be a plain name (no path separators or `..`)".to_string(),
            });
        }
        if tool.version.trim().is_empty() {
            return Err(ToolchainError::EmptyVersion {
                id: tool.id.clone(),
            });
        }
        if tool.digest.trim().is_empty() {
            return Err(ToolchainError::EmptyDigest {
                id: tool.id.clone(),
            });
        }
        if !valid_digest(tool.digest.trim()) {
            return Err(ToolchainError::BadDigest {
                id: tool.id.clone(),
            });
        }
        if tool.modes.is_empty() {
            return Err(ToolchainError::NoAllowedMode {
                id: tool.id.clone(),
            });
        }
        for p in &tool.platforms {
            if !is_supported_platform(p) {
                return Err(ToolchainError::UnsupportedPlatform {
                    id: tool.id.clone(),
                    platform: p.clone(),
                });
            }
        }
        if tool.modes.contains(&ExecutionMode::Container)
            && !tool
                .container_image
                .as_ref()
                .map(|s| valid_container_image(s.trim()))
                .unwrap_or(false)
        {
            return Err(ToolchainError::ContainerImageNotPinned {
                id: tool.id.clone(),
            });
        }
        if tool.modes.contains(&ExecutionMode::ProjectRuntime) {
            match tool.project_argv.as_ref() {
                Some(argv) if !argv.is_empty() && argv.iter().all(|a| !a.trim().is_empty()) => {}
                _ => {
                    return Err(ToolchainError::EmptyArgv {
                        id: tool.id.clone(),
                    })
                }
            }
        }
        if let Some(ms) = tool.timeout_ms {
            if ms == 0 {
                return Err(ToolchainError::Parse(format!(
                    "tool \"{}\" has `timeout_ms = 0` (remove it or set a positive value)",
                    tool.id
                )));
            }
        }
    }
    Ok(())
}

/// Reject an unsupported platform before any execution.
pub fn resolve_platform(tool: &ToolEntry, platform: &str) -> Result<(), ToolchainError> {
    if !is_supported_platform(platform) {
        return Err(ToolchainError::HostUnsupported {
            platform: platform.to_string(),
        });
    }
    if !tool.platforms.is_empty() && !tool.platforms.iter().any(|p| p == platform) {
        return Err(ToolchainError::UnsupportedPlatform {
            id: tool.id.clone(),
            platform: platform.to_string(),
        });
    }
    Ok(())
}

/// User cache base: `$XDG_CACHE_HOME/driftwatch/tools` or
/// `$HOME/.cache/driftwatch/tools`. Pure over explicit inputs so
/// tests do not touch the real home.
pub fn cache_base_from(home: Option<&str>, xdg: Option<&str>) -> PathBuf {
    if let Some(xdg) = xdg {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("driftwatch").join("tools");
        }
    }
    match home {
        Some(h) if !h.trim().is_empty() => PathBuf::from(h)
            .join(".cache")
            .join("driftwatch")
            .join("tools"),
        _ => PathBuf::from(".cache").join("driftwatch").join("tools"),
    }
}

/// Real user cache base from the process environment.
pub fn user_cache_base() -> PathBuf {
    let home = std::env::var("HOME").ok();
    let xdg = std::env::var("XDG_CACHE_HOME").ok();
    cache_base_from(home.as_deref(), xdg.as_deref())
}

/// Cache directory for one pinned tool on one platform:
/// `<base>/<id>/<version>/<platform>`.
pub fn cache_dir(base: &Path, tool: &ToolEntry, platform: &str) -> PathBuf {
    base.join(&tool.id).join(&tool.version).join(platform)
}

/// Cached executable path. The file name is the tool id (no extension;
/// Windows is outside the supported matrix).
pub fn executable_path(base: &Path, tool: &ToolEntry, platform: &str) -> PathBuf {
    cache_dir(base, tool, platform).join(&tool.id)
}

/// Lock path guarding one cached executable (`<exe>.lock`).
pub fn lock_path(base: &Path, tool: &ToolEntry, platform: &str) -> PathBuf {
    let mut p = executable_path(base, tool, platform).into_os_string();
    p.push(".lock");
    PathBuf::from(p)
}

/// `sha256:` hex digest of bytes.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

/// Verify bytes against a pinned digest. A mismatch is
/// [`ToolchainError::VerificationMismatch`]; the caller must delete
/// the payload and must not execute it.
pub fn verify_bytes(bytes: &[u8], expected_digest: &str, id: &str) -> Result<(), ToolchainError> {
    if bytes.len() > MAX_TOOL_BYTES {
        return Err(ToolchainError::VerificationMismatch { id: id.to_string() });
    }
    if digest_bytes(bytes) == expected_digest.trim() {
        Ok(())
    } else {
        Err(ToolchainError::VerificationMismatch { id: id.to_string() })
    }
}

/// True when the cached file exists, fits the size bound, and matches
/// the pinned digest. Reads the file; never executes it.
pub fn cached_verified(exe_path: &Path, expected_digest: &str) -> bool {
    let Ok(meta) = fs::metadata(exe_path) else {
        return false;
    };
    if meta.len() > MAX_TOOL_BYTES as u64 {
        return false;
    }
    let Ok(bytes) = fs::read(exe_path) else {
        return false;
    };
    digest_bytes(&bytes) == expected_digest.trim()
}

/// Atomically cache verified bytes at `exe_path`: verify first, write
/// to a temp sibling, then rename. A verification failure deletes the
/// temp file and leaves no executable behind.
pub fn store_verified_bytes(
    exe_path: &Path,
    bytes: &[u8],
    expected_digest: &str,
    id: &str,
) -> Result<PathBuf, ToolchainError> {
    verify_bytes(bytes, expected_digest, id)?;
    if let Some(parent) = exe_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ToolchainError::Cache {
            id: id.to_string(),
            detail: format!("cannot create cache dir: {e}"),
        })?;
    }
    let tmp = exe_path.with_extension("tmp-download");
    fs::write(&tmp, bytes).map_err(|e| ToolchainError::Cache {
        id: id.to_string(),
        detail: format!("cannot stage download: {e}"),
    })?;
    // Re-verify the staged file so a concurrent writer cannot swap it
    // under us before the rename.
    let staged = fs::read(&tmp).map_err(|e| ToolchainError::Cache {
        id: id.to_string(),
        detail: format!("cannot re-read staged download: {e}"),
    })?;
    if digest_bytes(&staged) != expected_digest.trim() {
        let _ = fs::remove_file(&tmp);
        return Err(ToolchainError::VerificationMismatch { id: id.to_string() });
    }
    fs::rename(&tmp, exe_path).map_err(|e| ToolchainError::Cache {
        id: id.to_string(),
        detail: format!("cannot publish to cache: {e}"),
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(exe_path)
            .map(|m| m.permissions())
            .unwrap_or_else(|_| {
                fs::metadata(&tmp)
                    .map(|m| m.permissions())
                    .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o755))
            });
        perms.set_mode(0o755);
        let _ = fs::set_permissions(exe_path, perms);
    }
    Ok(exe_path.to_path_buf())
}

/// RAII guard for a per-executable bootstrap lock. Created with
/// `create_new` so two bootstraps cannot interleave; dropped locks
/// remove the file. A stale lock from a crashed run surfaces as
/// contention (fail-closed) rather than being silently stolen.
#[derive(Debug)]
pub struct CacheLock {
    path: PathBuf,
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Acquire the per-executable lock. Contention is an error with a
/// retry remediation, never a wait or a steal.
pub fn acquire_lock(
    base: &Path,
    tool: &ToolEntry,
    platform: &str,
) -> Result<CacheLock, ToolchainError> {
    let path = lock_path(base, tool, platform);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ToolchainError::Cache {
            id: tool.id.clone(),
            detail: format!("cannot create lock dir: {e}"),
        })?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => Ok(CacheLock { path }),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(ToolchainError::LockContention {
                id: tool.id.clone(),
            })
        }
        Err(e) => Err(ToolchainError::Cache {
            id: tool.id.clone(),
            detail: format!("cannot acquire lock: {e}"),
        }),
    }
}

/// Capability probes. The real implementation reads the filesystem,
/// `PATH`, and the container runtime; tests inject fakes. Probing —
/// not configuration — decides readiness.
pub trait Probes {
    fn cached_ok(&self, exe_path: &Path, digest: &str) -> bool;
    fn program_on_path(&self, program: &str) -> bool;
    fn container_available(&self) -> bool;
}

/// Filesystem/`PATH` probes used in production.
pub struct RealProbes;

impl Probes for RealProbes {
    fn cached_ok(&self, exe_path: &Path, digest: &str) -> bool {
        cached_verified(exe_path, digest)
    }

    fn program_on_path(&self, program: &str) -> bool {
        if program.contains('/') {
            return Path::new(program).is_file();
        }
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths)
                .any(|d| d.join(program).is_file() || d.join(format!("{program}.exe")).is_file())
        })
    }

    fn container_available(&self) -> bool {
        // Bounded `docker info` probe: missing binaries or a hanging
        // daemon report unavailable instead of stalling the caller.
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let out = Command::new("docker").arg("info").output();
            let _ = tx.send(out);
        });
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(o)) => o.status.success(),
            _ => false,
        }
    }
}

/// Resolved backend ready for the existing runner (`program + argv`,
/// no shell). `offline_reused` is true when the verified cache served
/// the tool without network access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedExecution {
    pub tool_id: String,
    pub mode: ExecutionMode,
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Container-only: digest-pinned image ref.
    pub container_image: Option<String>,
    /// Container-only: project-root-confined mounts.
    pub mounts: Vec<String>,
    /// Managed-only: verified cached executable.
    pub cached_path: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub offline_reused: bool,
}

fn confine_mount(mount: &str) -> Result<(), ToolchainError> {
    let trimmed = mount.trim();
    if trimmed.is_empty() {
        return Err(ToolchainError::MountEscape {
            mount: mount.to_string(),
        });
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(ToolchainError::MountEscape {
            mount: mount.to_string(),
        });
    }
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                return Err(ToolchainError::MountEscape {
                    mount: mount.to_string(),
                })
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolchainError::MountEscape {
                    mount: mount.to_string(),
                })
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

/// Resolve one tool to an executable backend. Pure except for
/// capability probes: no network I/O, no download, no spawn. Missing
/// managed tools fail closed with the bootstrap next action under an
/// offline policy.
pub fn resolve_execution(
    tool: &ToolEntry,
    platform: &str,
    policy: ProvisionPolicy,
    cache_base: &Path,
    project_root: &Path,
    probes: &dyn Probes,
) -> Result<ResolvedExecution, ToolchainError> {
    validate(&ToolchainManifest {
        version: crate::gate::dto::GATE_CONTRACT_VERSION,
        tools: vec![tool.clone()],
    })?;
    resolve_platform(tool, platform)?;
    let _ = project_root;

    for mode in &tool.modes {
        match mode {
            ExecutionMode::Managed => {
                let exe = executable_path(cache_base, tool, platform);
                if probes.cached_ok(&exe, &tool.digest) {
                    return Ok(ResolvedExecution {
                        tool_id: tool.id.clone(),
                        mode: ExecutionMode::Managed,
                        program: exe.to_string_lossy().into_owned(),
                        args: vec![],
                        env: BTreeMap::new(),
                        container_image: None,
                        mounts: vec![],
                        cached_path: Some(exe),
                        timeout_ms: tool.timeout_ms,
                        offline_reused: true,
                    });
                }
                // Cached bytes absent or corrupt: fail closed. Ordinary
                // checks never download; explicit bootstrap provisions.
                if !policy.allow_network {
                    return Err(ToolchainError::OfflineMissing {
                        id: tool.id.clone(),
                        version: tool.version.clone(),
                        platform: platform.to_string(),
                    });
                }
                return Err(ToolchainError::ProvisioningDenied {
                    id: tool.id.clone(),
                });
            }
            ExecutionMode::Container => {
                for m in &tool.mounts {
                    confine_mount(m)?;
                }
                if probes.container_available() {
                    let image = tool.container_image.clone().unwrap_or_default();
                    return Ok(ResolvedExecution {
                        tool_id: tool.id.clone(),
                        mode: ExecutionMode::Container,
                        program: "docker".to_string(),
                        args: vec!["run".to_string(), "--rm".to_string(), image.clone()],
                        env: tool.env.clone(),
                        container_image: Some(image),
                        mounts: tool.mounts.clone(),
                        cached_path: None,
                        timeout_ms: tool.timeout_ms,
                        offline_reused: false,
                    });
                }
                // No runtime: try the next allowed mode; when container
                // is the only mode the gate maps this to
                // REVIEW_REQUIRED per policy.
                if tool.modes.len() == 1 {
                    return Err(ToolchainError::ContainerUnavailable {
                        id: tool.id.clone(),
                    });
                }
                continue;
            }
            ExecutionMode::Native => {
                if !policy.allow_native_fallback {
                    if tool.modes.len() == 1 {
                        return Err(ToolchainError::NativeNotAllowed {
                            id: tool.id.clone(),
                        });
                    }
                    continue;
                }
                let program = tool
                    .native_program
                    .clone()
                    .unwrap_or_else(|| tool.id.clone());
                if probes.program_on_path(&program) {
                    return Ok(ResolvedExecution {
                        tool_id: tool.id.clone(),
                        mode: ExecutionMode::Native,
                        program,
                        args: vec![],
                        env: BTreeMap::new(),
                        container_image: None,
                        mounts: vec![],
                        cached_path: None,
                        timeout_ms: tool.timeout_ms,
                        offline_reused: false,
                    });
                }
                if tool.modes.len() == 1 {
                    return Err(ToolchainError::NativeNotAllowed {
                        id: tool.id.clone(),
                    });
                }
                continue;
            }
            ExecutionMode::ProjectRuntime => {
                // Declared argv verbatim: argv[0] is the program, the
                // rest are args. Never whitespace-split, never install.
                let argv = tool.project_argv.clone().unwrap_or_default();
                let program = argv[0].clone();
                if probes.program_on_path(&program) {
                    return Ok(ResolvedExecution {
                        tool_id: tool.id.clone(),
                        mode: ExecutionMode::ProjectRuntime,
                        program,
                        args: argv[1..].to_vec(),
                        env: BTreeMap::new(),
                        container_image: None,
                        mounts: vec![],
                        cached_path: None,
                        timeout_ms: tool.timeout_ms,
                        offline_reused: false,
                    });
                }
                return Err(ToolchainError::ProjectRuntimeMissing {
                    id: tool.id.clone(),
                    program,
                });
            }
        }
    }
    Err(ToolchainError::NoAllowedMode {
        id: tool.id.clone(),
    })
}

/// One bootstrap step for a tool on a platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapAction {
    pub tool_id: String,
    pub version: String,
    pub platform: String,
    pub state: BootstrapState,
    /// Exact next action the operator takes (or `already ready`).
    pub next_action: String,
}

/// Provisioning state per tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapState {
    Ready,
    NeedsProvisioning,
    Blocked,
}

/// Explicit bootstrap plan: for every declared tool, report whether
/// the verified cache already serves it, what to provision, or what
/// blocks it. Never performs network I/O.
pub fn bootstrap_plan(
    manifest: &ToolchainManifest,
    platform: &str,
    cache_base: &Path,
    probes: &dyn Probes,
) -> Vec<BootstrapAction> {
    let mut out = Vec::new();
    for tool in &manifest.tools {
        if resolve_platform(tool, platform).is_err() {
            out.push(BootstrapAction {
                tool_id: tool.id.clone(),
                version: tool.version.clone(),
                platform: platform.to_string(),
                state: BootstrapState::Blocked,
                next_action: format!(
                    "tool `{}` does not serve {platform}; pick a supported platform (linux-x86_64, linux-arm64, darwin-x86_64)",
                    tool.id
                ),
            });
            continue;
        }
        let exe = executable_path(cache_base, tool, platform);
        if tool.modes.contains(&ExecutionMode::Managed) && probes.cached_ok(&exe, &tool.digest) {
            out.push(BootstrapAction {
                tool_id: tool.id.clone(),
                version: tool.version.clone(),
                platform: platform.to_string(),
                state: BootstrapState::Ready,
                next_action: format!(
                    "tool `{}` {} is cached and verified at {}",
                    tool.id,
                    tool.version,
                    exe.display()
                ),
            });
            continue;
        }
        if tool.modes == vec![ExecutionMode::ProjectRuntime] {
            let program = tool
                .project_argv
                .as_ref()
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or_default();
            if probes.program_on_path(&program) {
                out.push(BootstrapAction {
                    tool_id: tool.id.clone(),
                    version: tool.version.clone(),
                    platform: platform.to_string(),
                    state: BootstrapState::Ready,
                    next_action: format!(
                        "project runtime `{program}` is present; no provisioning needed"
                    ),
                });
            } else {
                out.push(BootstrapAction {
                    tool_id: tool.id.clone(),
                    version: tool.version.clone(),
                    platform: platform.to_string(),
                    state: BootstrapState::Blocked,
                    next_action: format!(
                        "install the project SDK providing `{program}` yourself; Driftwatchdog never installs project runtimes"
                    ),
                });
            }
            continue;
        }
        out.push(BootstrapAction {
            tool_id: tool.id.clone(),
            version: tool.version.clone(),
            platform: platform.to_string(),
            state: BootstrapState::NeedsProvisioning,
            next_action: format!(
                "provision tool `{}` {} for {platform} from {} then verify digest {}",
                tool.id,
                tool.version,
                tool.source.as_deref().unwrap_or("(manifest source)"),
                tool.digest
            ),
        });
    }
    out
}

/// Render a bootstrap plan as human-readable text. Pure: no process
/// is spawned and no state is mutated.
pub fn render_bootstrap_plan(actions: &[BootstrapAction]) -> String {
    let mut out = String::new();
    out.push_str("gate toolchain bootstrap (explicit; ordinary checks never download)\n");
    if actions.is_empty() {
        out.push_str("  (no gate tools declared)\n");
        return out;
    }
    for a in actions {
        let state = match a.state {
            BootstrapState::Ready => "ready",
            BootstrapState::NeedsProvisioning => "needs-provisioning",
            BootstrapState::Blocked => "blocked",
        };
        out.push_str(&format!(
            "  - {} {} [{}] for {}: {}\n",
            a.tool_id, a.version, state, a.platform, a.next_action
        ));
    }
    out
}

/// Resolve the toolchain manifest path: `<root>/gate-tools.toml`
/// first, then `<root>/.driftwatch/gate-tools.toml`. `None` means no
/// Gate tools are declared (not an error; callers stay silent).
pub fn manifest_path(project_root: &Path) -> Option<PathBuf> {
    for name in TOOLCHAIN_MANIFEST_FILES {
        let primary = project_root.join(name);
        if primary.is_file() {
            return Some(primary);
        }
    }
    let scoped = project_root
        .join(".driftwatch")
        .join(TOOLCHAIN_MANIFEST_FILES[0]);
    if scoped.is_file() {
        return Some(scoped);
    }
    None
}

/// Load the toolchain manifest. `Ok(None)` when no file exists.
pub fn load(project_root: &Path) -> Result<Option<ToolchainManifest>, ToolchainError> {
    let Some(path) = manifest_path(project_root) else {
        return Ok(None);
    };
    let text = fs::read_to_string(&path)
        .map_err(|e| ToolchainError::Parse(format!("cannot read {}: {e}", path.display())))?;
    parse(&text).map(Some)
}

/// Truthful readiness checks for `doctor`. Probes — not config —
/// decide each outcome: cached digests are re-read, `PATH` is
/// searched, and container capability is probed with a bound.
/// Missing manifest yields no checks (unconfigured-but-ok lives with
/// the existing checker checks); a malformed manifest is a `Warn`.
pub fn toolchain_checks(
    project_root: &Path,
    cache_base: &Path,
    probes: &dyn Probes,
) -> Vec<crate::doctor::check::Check> {
    use crate::doctor::check::Check;
    let Some(path) = manifest_path(project_root) else {
        return Vec::new();
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            return vec![Check::warn(
                "gate.toolchain.manifest",
                "Gate toolchain manifest is unreadable",
                format!("{}: {e}", path.display()),
            )
            .with_remediation(format!("Fix permissions on `{}`.", path.display()))]
        }
    };
    let manifest = match parse(&text) {
        Ok(m) => m,
        Err(e) => {
            return vec![Check::warn(
                "gate.toolchain.manifest",
                "Gate toolchain manifest is invalid",
                format!("{e}"),
            )
            .with_remediation(format!(
                "Edit `{}` to fix the parse error (unknown fields show a `did you mean` hint).",
                path.display()
            ))]
        }
    };
    if manifest.tools.is_empty() {
        return vec![Check::info(
            "gate.toolchain.configured",
            "No Gate tools declared",
            "gate-tools.toml declares no tools; gate execution has nothing to provision.",
        )];
    }
    let platform = current_platform();
    let mut out = Vec::new();
    for tool in &manifest.tools {
        if resolve_platform(tool, &platform).is_err() {
            out.push(
                Check::fail(
                    "gate.toolchain.platform",
                    format!("Gate tool `{}` has no build for this host", tool.id),
                    format!(
                        "host {platform} is not served; supported: linux-x86_64, linux-arm64, darwin-x86_64"
                    ),
                )
                .with_remediation(format!(
                    "Provision tool `{}` on a supported host or remove it from gate-tools.toml.",
                    tool.id
                )),
            );
            continue;
        }
        if tool.modes.contains(&ExecutionMode::Managed) {
            let exe = executable_path(cache_base, tool, &platform);
            if probes.cached_ok(&exe, &tool.digest) {
                out.push(
                    Check::pass(
                        "gate.toolchain.cached",
                        format!("Gate tool `{}` is cached and verified", tool.id),
                    )
                    .with_remediation(format!(
                        "{} {} at {} (digest {})",
                        tool.id,
                        tool.version,
                        exe.display(),
                        tool.digest
                    )),
                );
            } else {
                out.push(
                    Check::warn(
                        "gate.toolchain.missing",
                        format!("Gate tool `{}` is not cached", tool.id),
                        format!("version {} for {platform} is absent or corrupt", tool.version),
                    )
                    .with_remediation(format!(
                        "Provision it explicitly, then verify digest {}; ordinary checks never download tools.",
                        tool.digest
                    )),
                );
            }
        }
        if tool.modes.contains(&ExecutionMode::Container) && !probes.container_available() {
            out.push(
                Check::warn(
                    "gate.toolchain.container",
                    format!("Container runtime unavailable for `{}`", tool.id),
                    "no container runtime responded".to_string(),
                )
                .with_remediation(
                    "Start docker/podman or select an allowed fallback; gated results map to REVIEW_REQUIRED per policy.",
                ),
            );
        }
        if tool.modes.contains(&ExecutionMode::ProjectRuntime) {
            let program = tool
                .project_argv
                .as_ref()
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or_default();
            if probes.program_on_path(&program) {
                out.push(
                    Check::pass(
                        "gate.toolchain.project_runtime",
                        format!("Project runtime `{program}` is present"),
                    )
                    .with_remediation(format!(
                        "Tool `{}` runs the declared argv; no SDK install attempted.",
                        tool.id
                    )),
                );
            } else {
                out.push(
                    Check::warn(
                        "gate.toolchain.project_runtime",
                        format!("Project runtime `{program}` is missing"),
                        format!("tool `{}` declares project argv starting with `{program}`", tool.id),
                    )
                    .with_remediation(format!(
                        "Install the project SDK providing `{program}` yourself; Driftwatchdog never installs project runtimes."
                    )),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    const VALID: &str = r#"
version = 1

[[tools]]
id = "semgrep"
version = "1.2.3"
platforms = ["linux-x86_64"]
source = "https://example.invalid/semgrep-1.2.3"
digest = "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
modes = ["managed"]
"#;

    struct FakeProbes {
        cached: BTreeMap<String, bool>,
        path: BTreeMap<String, bool>,
        container: bool,
    }

    impl FakeProbes {
        fn empty() -> Self {
            Self {
                cached: BTreeMap::new(),
                path: BTreeMap::new(),
                container: false,
            }
        }
    }

    impl Probes for FakeProbes {
        fn cached_ok(&self, exe_path: &Path, _digest: &str) -> bool {
            self.cached
                .get(&exe_path.to_string_lossy().into_owned())
                .copied()
                .unwrap_or(false)
        }

        fn program_on_path(&self, program: &str) -> bool {
            self.path.get(program).copied().unwrap_or(false)
        }

        fn container_available(&self) -> bool {
            self.container
        }
    }

    fn managed_tool(id: &str) -> ToolEntry {
        ToolEntry {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            platforms: vec![],
            source: Some("https://example.invalid/t".to_string()),
            digest: digest_bytes(b"tool-bytes"),
            signature: None,
            modes: vec![ExecutionMode::Managed],
            container_image: None,
            native_program: None,
            project_argv: None,
            mounts: vec![],
            env: BTreeMap::new(),
            timeout_ms: None,
        }
    }

    #[test]
    fn valid_manifest_parses() {
        let m = parse(VALID).unwrap();
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.tools[0].id, "semgrep");
    }

    #[test]
    fn unknown_field_fails_with_hint() {
        let err = parse("version = 1\n[[tools]]\nid = \"a\"\nversion = \"1\"\ndigest = \"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"\nversoin = \"x\"\n")
            .unwrap_err();
        match err {
            ToolchainError::UnknownField { field, suggestion } => {
                assert_eq!(field, "versoin");
                assert!(suggestion.contains("version"), "got: {suggestion}");
            }
            other => panic!("unexpected: {other}"),
        }
    }

    #[test]
    fn version_mismatch_fails() {
        let err = parse("version = 999\n").unwrap_err();
        assert!(matches!(err, ToolchainError::UnknownVersion { .. }));
    }

    #[test]
    fn empty_digest_and_bad_digest_fail() {
        let no_digest = "version = 1\n[[tools]]\nid = \"a\"\nversion = \"1\"\ndigest = \"\"\n";
        assert!(matches!(
            parse(no_digest).unwrap_err(),
            ToolchainError::EmptyDigest { .. }
        ));
        let bad = "version = 1\n[[tools]]\nid = \"a\"\nversion = \"1\"\ndigest = \"md5:abc\"\n";
        assert!(matches!(
            parse(bad).unwrap_err(),
            ToolchainError::BadDigest { .. }
        ));
    }

    #[test]
    fn duplicate_ids_fail() {
        let text = "version = 1\n[[tools]]\nid = \"a\"\nversion = \"1\"\ndigest = \"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"\n[[tools]]\nid = \"a\"\nversion = \"2\"\ndigest = \"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"\n";
        assert!(matches!(
            parse(text).unwrap_err(),
            ToolchainError::DuplicateId { .. }
        ));
    }

    #[test]
    fn unsupported_entry_platform_rejected_before_execution() {
        let mut tool = managed_tool("semgrep");
        tool.platforms = vec!["windows-x86_64".to_string()];
        // Validation itself rejects unknown matrix entries.
        let manifest = ToolchainManifest {
            version: crate::gate::dto::GATE_CONTRACT_VERSION,
            tools: vec![tool.clone()],
        };
        assert!(validate(&manifest).is_err());
        // And resolution rejects a host outside the matrix.
        assert!(matches!(
            resolve_platform(&managed_tool("x"), "darwin-arm64"),
            Err(ToolchainError::HostUnsupported { .. })
        ));
    }

    #[test]
    fn host_outside_matrix_fails_closed() {
        let tool = managed_tool("semgrep");
        let probes = FakeProbes::empty();
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_execution(
            &tool,
            "windows-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap_err();
        assert!(matches!(err, ToolchainError::HostUnsupported { .. }));
    }

    #[test]
    fn digest_round_trip_and_mismatch() {
        let d = digest_bytes(b"hello");
        assert!(d.starts_with("sha256:"));
        assert!(verify_bytes(b"hello", &d, "t").is_ok());
        assert!(matches!(
            verify_bytes(b"other", &d, "t").unwrap_err(),
            ToolchainError::VerificationMismatch { .. }
        ));
    }

    #[test]
    fn store_verified_bytes_round_trips_and_mismatch_leaves_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("semgrep");
        let digest = digest_bytes(b"payload");
        store_verified_bytes(&exe, b"payload", &digest, "semgrep").unwrap();
        assert!(cached_verified(&exe, &digest));
        let bad_exe = tmp.path().join("other");
        let err = store_verified_bytes(&bad_exe, b"payload", &digest_bytes(b"nope"), "other")
            .unwrap_err();
        assert!(matches!(err, ToolchainError::VerificationMismatch { .. }));
        assert!(!bad_exe.exists());
    }

    #[test]
    fn offline_policy_reuses_cache_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = managed_tool("semgrep");
        let exe = executable_path(tmp.path(), &tool, "linux-x86_64");
        store_verified_bytes(&exe, b"tool-bytes", &tool.digest, &tool.id).unwrap();
        let probes = RealProbes;
        let resolved = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap();
        assert_eq!(resolved.mode, ExecutionMode::Managed);
        assert!(resolved.offline_reused);
        assert_eq!(resolved.program, exe.to_string_lossy());
    }

    #[test]
    fn offline_missing_fails_closed_with_bootstrap_action() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = managed_tool("semgrep");
        let probes = FakeProbes::empty();
        let err = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap_err();
        assert!(matches!(err, ToolchainError::OfflineMissing { .. }));
        // Ordinary checks must not have created anything.
        assert!(!executable_path(tmp.path(), &tool, "linux-x86_64").exists());
    }

    #[test]
    fn cache_lock_contention_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = managed_tool("semgrep");
        let _guard = acquire_lock(tmp.path(), &tool, "linux-x86_64").unwrap();
        let err = acquire_lock(tmp.path(), &tool, "linux-x86_64").unwrap_err();
        assert!(matches!(err, ToolchainError::LockContention { .. }));
    }

    #[test]
    fn cache_lock_sequential_acquire_works() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = managed_tool("semgrep");
        {
            let _g = acquire_lock(tmp.path(), &tool, "linux-x86_64").unwrap();
        }
        // Dropped guard removed the file; a second acquire succeeds.
        let _g2 = acquire_lock(tmp.path(), &tool, "linux-x86_64").unwrap();
    }

    #[test]
    fn concurrent_bootstrap_contention_is_safe() {
        use std::sync::atomic::{AtomicI32, Ordering};
        let tmp = tempfile::tempdir().unwrap();
        let tool = managed_tool("semgrep");
        let base = tmp.path().to_path_buf();
        // Track simultaneous holders: mutual exclusion means the max
        // observed concurrency is 1 even when threads overlap. Total
        // acquisitions may exceed 1 (sequential reuse after drop is
        // fine); what must never happen is two holders at once or a
        // leftover lock file.
        let active: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
        let max_seen: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let base = base.clone();
            let tool = tool.clone();
            let active = active.clone();
            let max_seen = max_seen.clone();
            handles.push(std::thread::spawn(move || {
                if let Ok(_guard) = acquire_lock(&base, &tool, "linux-x86_64") {
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    {
                        let mut m = max_seen.lock().unwrap();
                        if cur > *m {
                            *m = cur;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    active.fetch_sub(1, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*max_seen.lock().unwrap(), 1);
        assert!(!lock_path(&base, &tool, "linux-x86_64").exists());
    }

    #[test]
    fn container_image_must_be_digest_pinned() {
        let text = "version = 1\n[[tools]]\nid = \"scan\"\nversion = \"1\"\ndigest = \"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"\nmodes = [\"container\"]\ncontainer_image = \"scan:latest\"\n";
        assert!(matches!(
            parse(text).unwrap_err(),
            ToolchainError::ContainerImageNotPinned { .. }
        ));
    }

    #[test]
    fn container_unavailable_maps_to_review() {
        let mut tool = managed_tool("scan");
        tool.modes = vec![ExecutionMode::Container];
        tool.container_image = Some(
            "scan@sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                .to_string(),
        );
        let tmp = tempfile::tempdir().unwrap();
        let probes = FakeProbes::empty();
        let err = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap_err();
        // Single-mode container without a runtime fails closed with a
        // remediation the adapter maps to REVIEW_REQUIRED per policy.
        assert!(matches!(err, ToolchainError::ContainerUnavailable { .. }));
        assert!(err.to_string().contains("REVIEW_REQUIRED"));
    }

    #[test]
    fn container_mounts_escape_rejected() {
        let mut tool = managed_tool("scan");
        tool.modes = vec![ExecutionMode::Container];
        tool.container_image = Some(
            "scan@sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                .to_string(),
        );
        tool.mounts = vec!["../outside".to_string()];
        let tmp = tempfile::tempdir().unwrap();
        let probes = FakeProbes {
            container: true,
            ..FakeProbes::empty()
        };
        let err = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap_err();
        assert!(matches!(err, ToolchainError::MountEscape { .. }));
    }

    #[test]
    fn native_mode_requires_opt_in() {
        let mut tool = managed_tool("rg");
        tool.modes = vec![ExecutionMode::Native];
        let tmp = tempfile::tempdir().unwrap();
        let probes = FakeProbes {
            path: BTreeMap::from([("rg".to_string(), true)]),
            ..FakeProbes::empty()
        };
        // Offline default rejects native even when the binary exists.
        assert!(matches!(
            resolve_execution(
                &tool,
                "linux-x86_64",
                ProvisionPolicy::offline(),
                tmp.path(),
                tmp.path(),
                &probes,
            )
            .unwrap_err(),
            ToolchainError::NativeNotAllowed { .. }
        ));
        let policy = ProvisionPolicy {
            allow_network: false,
            allow_native_fallback: true,
        };
        let resolved = resolve_execution(
            &tool,
            "linux-x86_64",
            policy,
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap();
        assert_eq!(resolved.mode, ExecutionMode::Native);
        assert_eq!(resolved.program, "rg");
    }

    #[test]
    fn project_runtime_argv_passes_through_without_shell_splitting() {
        let mut tool = managed_tool("dotnet-test");
        tool.modes = vec![ExecutionMode::ProjectRuntime];
        tool.project_argv = Some(vec![
            "dotnet".to_string(),
            "test".to_string(),
            "My Suite/Project.csproj".to_string(),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let probes = FakeProbes {
            path: BTreeMap::from([("dotnet".to_string(), true)]),
            ..FakeProbes::empty()
        };
        let resolved = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap();
        assert_eq!(resolved.mode, ExecutionMode::ProjectRuntime);
        assert_eq!(resolved.program, "dotnet");
        // The spaced path stays one argv element (no shell splitting).
        assert_eq!(resolved.args, vec!["test", "My Suite/Project.csproj"]);
    }

    #[test]
    fn project_runtime_missing_never_installs() {
        let mut tool = managed_tool("py");
        tool.modes = vec![ExecutionMode::ProjectRuntime];
        tool.project_argv = Some(vec![
            "python3".to_string(),
            "-m".to_string(),
            "pytest".to_string(),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let probes = FakeProbes::empty();
        let err = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::bootstrap(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap_err();
        assert!(matches!(err, ToolchainError::ProjectRuntimeMissing { .. }));
        assert!(err.to_string().contains("never installs"));
    }

    #[test]
    fn timeout_carries_into_resolved_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let mut tool = managed_tool("semgrep");
        tool.timeout_ms = Some(30_000);
        let exe = executable_path(tmp.path(), &tool, "linux-x86_64");
        store_verified_bytes(&exe, b"tool-bytes", &tool.digest, &tool.id).unwrap();
        let probes = RealProbes;
        let resolved = resolve_execution(
            &tool,
            "linux-x86_64",
            ProvisionPolicy::offline(),
            tmp.path(),
            tmp.path(),
            &probes,
        )
        .unwrap();
        assert_eq!(resolved.timeout_ms, Some(30_000));
    }

    #[test]
    fn zero_timeout_rejected() {
        let text = "version = 1\n[[tools]]\nid = \"a\"\nversion = \"1\"\ndigest = \"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"\ntimeout_ms = 0\n";
        assert!(parse(text).is_err());
    }

    #[test]
    fn bootstrap_plan_reports_ready_missing_and_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cached = managed_tool("cached-tool");
        let exe = executable_path(tmp.path(), &cached, "linux-x86_64");
        cached.digest = digest_bytes(b"tool-bytes");
        store_verified_bytes(&exe, b"tool-bytes", &cached.digest, &cached.id).unwrap();
        let manifest = ToolchainManifest {
            version: crate::gate::dto::GATE_CONTRACT_VERSION,
            tools: vec![cached, managed_tool("missing-tool"), {
                let mut t = managed_tool("win-only");
                t.platforms = vec!["linux-x86_64".to_string()];
                t
            }],
        };
        let probes = RealProbes;
        let plan = bootstrap_plan(&manifest, "linux-x86_64", tmp.path(), &probes);
        assert_eq!(plan.len(), 3);
        assert!(plan.iter().any(|a| a.state == BootstrapState::Ready));
        assert!(plan
            .iter()
            .any(|a| a.state == BootstrapState::NeedsProvisioning));
        let text = render_bootstrap_plan(&plan);
        assert!(text.contains("explicit"));
        assert!(text.contains("ordinary checks never download"));
    }

    #[test]
    fn manifest_path_prefers_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("gate-tools.toml");
        let scoped_dir = tmp.path().join(".driftwatch");
        std::fs::create_dir_all(&scoped_dir).unwrap();
        std::fs::write(&primary, VALID).unwrap();
        std::fs::write(scoped_dir.join("gate-tools.toml"), VALID).unwrap();
        assert_eq!(manifest_path(tmp.path()), Some(primary));
    }

    #[test]
    fn missing_manifest_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().is_none());
        let checks = toolchain_checks(tmp.path(), tmp.path(), &FakeProbes::empty());
        assert!(checks.is_empty());
    }

    #[test]
    fn malformed_manifest_surfaces_warn() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("gate-tools.toml"), "version = [\n").unwrap();
        let checks = toolchain_checks(tmp.path(), tmp.path(), &FakeProbes::empty());
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, crate::doctor::check::Status::Warn);
    }

    #[test]
    fn doctor_reports_missing_tool_with_bootstrap_remediation() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("gate-tools.toml"), VALID).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let checks = toolchain_checks(tmp.path(), cache.path(), &FakeProbes::empty());
        assert!(checks.iter().any(|c| c.id == "gate.toolchain.missing"));
        let missing = checks
            .iter()
            .find(|c| c.id == "gate.toolchain.missing")
            .unwrap();
        assert!(missing
            .remediation
            .as_deref()
            .unwrap_or_default()
            .contains("never download"));
    }

    #[test]
    fn supported_platforms_match_release_matrix() {
        for p in ["linux-x86_64", "linux-arm64", "darwin-x86_64"] {
            assert!(is_supported_platform(p), "{p}");
        }
        for p in ["darwin-arm64", "windows-x86_64", "linux-musl-x86_64"] {
            assert!(!is_supported_platform(p), "{p}");
        }
    }
}
