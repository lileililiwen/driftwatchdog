//! Generic gate context providers.
//!
//! Gate planning and evaluation need change context, but not every
//! project uses OpenSpec. Providers therefore load **generic**
//! [`ContextDocument`] values (provider, kind, path, digest, size,
//! optional change id, redacted bounded preview) while the core never
//! depends on OpenSpec or any other specification-system types.
//!
//! ## Providers
//!
//! * `git` — read-only `status --porcelain` plus `diff` names/content,
//!   bounded per command (~5s). Outside a worktree the provider yields
//!   unavailable markers (`NOT_APPLICABLE`), never a fabricated context.
//! * `project-files` — caller-selected relative files confined to the
//!   project root (e.g. `gate.toml`, `driftwatch.toml`).
//! * `openspec` — optional adapter that reads `openspec/` markdown
//!   (`proposal.md`, `design.md`, `tasks.md`, `spec.md` under changes and
//!   top-level specs) as plain text. It parses no OpenSpec schema and
//!   imports no OpenSpec types; missing files yield unavailable markers.
//!
//! ## Bounds
//!
//! Collection never loads unbounded content: per-document bytes are
//! capped ([`MAX_CONTEXT_BYTES`]), document count is capped
//! ([`MAX_CONTEXT_DOCS`]), previews are secret-redacted and truncated
//! at a UTF-8 char boundary ([`MAX_PREVIEW_BYTES`]). Oversized input is
//! recorded as truncated/unavailable, never silently dropped. Providers
//! are read-only: only `fs::read` and read-only `git` subcommands run;
//! path escapes fail safely without reading the target.
//!
//! No tool is installed, no network call is made, and no LLM is invoked.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::gate::manifest::GateManifest;
use crate::gate::redact::bound_text_with_extra;
use crate::util::truncate_char_boundary;

/// Largest single context document accepted (64 KiB).
pub const MAX_CONTEXT_BYTES: usize = 64 * 1024;
/// Largest number of documents per collection.
pub const MAX_CONTEXT_DOCS: usize = 32;
/// Largest redacted preview per document.
pub const MAX_PREVIEW_BYTES: usize = 1024;
/// Largest accepted document path (bytes).
pub const MAX_PATH_BYTES: usize = 512;
/// Marker served when context content is absent or was truncated away.
pub const UNAVAILABLE_PREVIEW: &str = "[context unavailable]";
/// Budget per `git` invocation so a hanging binary cannot stall planning.
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Well-known provider names (opaque to the core schema).
pub const PROVIDER_GIT: &str = "git";
pub const PROVIDER_PROJECT_FILES: &str = "project-files";
pub const PROVIDER_OPENSPEC: &str = "openspec";

/// Well-known document kinds. Kinds are plain strings so future
/// providers (Gherkin, Markdown, Jira, ...) extend the set without
/// changing the core.
pub const KIND_GIT_STATUS: &str = "git-status";
pub const KIND_GIT_DIFF: &str = "git-diff";
pub const KIND_PROJECT_FILE: &str = "project-file";
pub const KIND_OPENSPEC_DOC: &str = "openspec-doc";

/// One bounded, hashed, generic context document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDocument {
    /// Provider that produced this document (`git`, `openspec`, ...).
    pub provider: String,
    /// Generic kind (`git-status`, `project-file`, `openspec-doc`, ...).
    pub kind: String,
    /// Project-root-relative path with forward slashes, or a synthetic
    /// id (`git/status`) for non-file context.
    pub path: String,
    /// `sha256:` hex digest over the stored bytes.
    pub digest: String,
    /// Stored byte size.
    pub byte_size: u64,
    /// True when the source exceeded the byte cap and only a prefix
    /// was stored.
    pub truncated: bool,
    /// False for missing/unreadable/escaped sources. Unavailable
    /// documents carry no content and must never be presented as
    /// satisfied requirements.
    pub available: bool,
    /// Redacted, bounded preview (`None` when unavailable).
    pub preview: Option<String>,
    /// Optional change/revision identity supplied by the caller.
    pub change_id: Option<String>,
}

impl ContextDocument {
    /// Marker for context that cannot be safely loaded. The reference
    /// stays auditable; content is explicitly unavailable.
    pub fn unavailable(provider: &str, kind: &str, path: &str) -> Self {
        Self {
            provider: provider.to_string(),
            kind: kind.to_string(),
            path: truncate_char_boundary(path, MAX_PATH_BYTES),
            digest: "sha256:unavailable".to_string(),
            byte_size: 0,
            truncated: false,
            available: false,
            preview: None,
            change_id: None,
        }
    }

    /// Safe preview for consumers: redacted bounded text while
    /// available, otherwise the unavailable marker. Never raw bytes.
    pub fn safe_preview(&self) -> &str {
        if self.available {
            self.preview.as_deref().unwrap_or(UNAVAILABLE_PREVIEW)
        } else {
            UNAVAILABLE_PREVIEW
        }
    }
}

/// Failure modes for context collection. Messages are safe
/// diagnostics: paths and policy, never raw content.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ContextError {
    #[error("unknown context provider \"{name}\"; expected one of: git, project-files, openspec")]
    UnknownProvider { name: String },
    #[error(
        "context path \"{path}\" escapes the project root; providers only read inside the project"
    )]
    PathEscape { path: String },
    #[error("io error loading context \"{path}\": {message}")]
    Io { path: String, message: String },
}

/// Caller-controlled collection options. `extra_files` feeds the
/// `project-files` provider; `sensitive` carries configured secret
/// values redacted in addition to recognized patterns.
#[derive(Debug, Clone, Default)]
pub struct ContextOptions {
    pub extra_files: Vec<String>,
    pub sensitive: Vec<String>,
    pub change_id: Option<String>,
    pub max_bytes_per_doc: Option<usize>,
    pub max_docs: Option<usize>,
}

impl ContextOptions {
    fn byte_cap(&self) -> usize {
        self.max_bytes_per_doc
            .unwrap_or(MAX_CONTEXT_BYTES)
            .min(MAX_CONTEXT_BYTES * 4)
    }

    fn doc_cap(&self) -> usize {
        self.max_docs
            .unwrap_or(MAX_CONTEXT_DOCS)
            .min(MAX_CONTEXT_DOCS * 2)
    }
}

/// Collected context: bounded documents plus human-readable
/// diagnostics (one line per provider outcome).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextBundle {
    pub docs: Vec<ContextDocument>,
    pub diagnostics: Vec<String>,
}

impl ContextBundle {
    pub fn available_count(&self) -> usize {
        self.docs.iter().filter(|d| d.available).count()
    }

    pub fn unavailable_count(&self) -> usize {
        self.docs.iter().filter(|d| !d.available).count()
    }

    /// Render bundle diagnostics as human-readable text. Pure: no
    /// child process is spawned and no state is mutated.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "context: {} available, {} unavailable ({} documents)\n",
            self.available_count(),
            self.unavailable_count(),
            self.docs.len()
        ));
        for d in &self.docs {
            out.push_str(&format!(
                "  - {}:{} {} [{}]{} {}\n",
                d.provider,
                d.kind,
                d.path,
                d.digest,
                if d.truncated { " (truncated)" } else { "" },
                if d.available {
                    "available"
                } else {
                    "NOT_APPLICABLE"
                },
            ));
        }
        if !self.diagnostics.is_empty() {
            out.push_str("diagnostics:\n");
            for line in &self.diagnostics {
                out.push_str(&format!("  - {line}\n"));
            }
        }
        out
    }
}

/// A read-only context provider. Implementations only read files under
/// the project root or run read-only `git` subcommands; they never
/// mutate project state. Collection is infallible by construction:
/// failures yield unavailable documents, never an abort.
pub trait ContextProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn collect(&self, root: &Path, opts: &ContextOptions) -> Vec<ContextDocument>;
}

/// Registry of known providers. Unknown names are reported (not
/// executed); one provider failure never aborts the rest.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<(&'static str, Box<dyn ContextProvider>)>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Registry with the three built-in providers.
    pub fn standard() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(GitProvider));
        reg.register(Box::new(ProjectFilesProvider));
        reg.register(Box::new(OpenspecProvider));
        reg
    }

    pub fn register(&mut self, provider: Box<dyn ContextProvider>) {
        let name = provider.name();
        if !self.providers.iter().any(|(n, _)| *n == name) {
            self.providers.push((name, provider));
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.providers.iter().any(|(n, _)| *n == name)
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.providers.iter().map(|(n, _)| *n).collect()
    }

    fn get(&self, name: &str) -> Option<&dyn ContextProvider> {
        self.providers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| p.as_ref())
    }
}

/// Ordered, de-duplicated provider selection from a Gate manifest.
/// Unknown names are preserved so collection can report them.
pub fn selection_from_manifest(manifest: &GateManifest) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for ctx in &manifest.contexts {
        let name = ctx.provider.trim().to_string();
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        out.push(name);
    }
    out
}

/// Collect context for `selection` (provider names in order) under
/// `root`. Unknown providers yield an unavailable document plus a
/// diagnostic; an empty selection yields an empty (valid) bundle so
/// reports and exports stay valid without configured providers.
pub fn collect_context(root: &Path, selection: &[String], opts: &ContextOptions) -> ContextBundle {
    collect_with_registry(root, selection, opts, &ProviderRegistry::standard())
}

fn collect_with_registry(
    root: &Path,
    selection: &[String],
    opts: &ContextOptions,
    registry: &ProviderRegistry,
) -> ContextBundle {
    let mut bundle = ContextBundle::default();
    let cap = opts.doc_cap();
    for name in selection {
        if bundle.docs.len() >= cap {
            bundle.diagnostics.push(format!(
                "document cap ({cap}) reached; remaining providers skipped"
            ));
            break;
        }
        match registry.get(name) {
            Some(provider) => {
                let before = bundle.docs.len();
                for mut doc in provider.collect(root, opts) {
                    if bundle.docs.len() >= cap {
                        break;
                    }
                    if opts.change_id.is_some() && doc.change_id.is_none() {
                        doc.change_id = opts.change_id.clone();
                    }
                    bundle.docs.push(doc);
                }
                let produced = bundle.docs.len() - before;
                bundle
                    .diagnostics
                    .push(format!("provider `{name}` produced {produced} document(s)"));
            }
            None => {
                bundle
                    .docs
                    .push(ContextDocument::unavailable(name, "unknown", name));
                bundle.diagnostics.push(format!(
                    "unknown context provider \"{name}\"; expected one of: git, project-files, openspec"
                ));
            }
        }
    }
    bundle
}

/// Validate a caller-supplied relative path and join it under `root`.
/// Absolute paths and `..` escapes are rejected without reading.
fn confine_path(root: &Path, supplied: &str) -> Result<PathBuf, ContextError> {
    let escape = || ContextError::PathEscape {
        path: supplied.chars().take(128).collect(),
    };
    let supplied_path = Path::new(supplied);
    if supplied_path.is_absolute() {
        return Err(escape());
    }
    let mut rel = PathBuf::new();
    for comp in supplied_path.components() {
        match comp {
            Component::Normal(part) => rel.push(part),
            Component::CurDir => {}
            _ => return Err(escape()),
        }
    }
    if rel.as_os_str().is_empty() {
        return Err(escape());
    }
    Ok(root.join(&rel))
}

fn rel_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// `sha256:` digest of stored bytes as lowercase hex.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sum = hasher.finalize();
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for b in sum {
        out.push(HEX[(b >> 4) as usize]);
        out.push(HEX[(b & 0x0f) as usize]);
    }
    out
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn sensitive_refs(opts: &ContextOptions) -> Vec<&str> {
    opts.sensitive.iter().map(String::as_str).collect()
}

/// Build a document from in-memory bytes: digest over stored bytes,
/// redacted bounded preview. Content beyond `cap` is cut at a UTF-8
/// char boundary and flagged `truncated`.
fn build_doc(
    provider: &str,
    kind: &str,
    path: &str,
    content: &[u8],
    cap: usize,
    preview_cap: usize,
    opts: &ContextOptions,
) -> ContextDocument {
    let truncated = content.len() > cap;
    // Raw char-boundary cut without ellipsis: stored bytes stay within
    // `cap` (the preview layer adds its own marker within its cap).
    let stored_text: std::borrow::Cow<'_, str> = if truncated {
        let text = String::from_utf8_lossy(content);
        let mut cut = cap.min(text.len());
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        std::borrow::Cow::Owned(text[..cut].to_string())
    } else {
        String::from_utf8_lossy(content)
    };
    let stored = stored_text.as_bytes();
    let digest = digest_bytes(stored);
    let preview = bound_text_with_extra(&stored_text, &sensitive_refs(opts), preview_cap);
    ContextDocument {
        provider: provider.to_string(),
        kind: kind.to_string(),
        path: truncate_char_boundary(path, MAX_PATH_BYTES),
        digest,
        byte_size: stored.len() as u64,
        truncated,
        available: true,
        preview: Some(preview),
        change_id: opts.change_id.clone(),
    }
}

/// Load one project-root-confined file into a document. Missing files
/// yield unavailable markers (optional context is `NOT_APPLICABLE`,
/// never false success); escapes fail safely without reading.
fn load_confined_file(
    root: &Path,
    provider: &str,
    kind: &str,
    supplied: &str,
    opts: &ContextOptions,
) -> ContextDocument {
    let cap = opts.byte_cap();
    let path = match confine_path(root, supplied) {
        Ok(p) => p,
        Err(_) => {
            return ContextDocument::unavailable(
                provider,
                kind,
                &truncate_char_boundary(supplied, MAX_PATH_BYTES),
            );
        }
    };
    let rel = rel_display(root, &path);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return ContextDocument::unavailable(provider, kind, &rel),
    };
    build_doc(provider, kind, &rel, &bytes, cap, MAX_PREVIEW_BYTES, opts)
}

fn run_git(root: &Path, args: &[&str]) -> Option<String> {
    let root = root.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new("git").args(&args).current_dir(&root).output();
        let _ = tx.send(out);
    });
    let output = match rx.recv_timeout(GIT_TIMEOUT) {
        Ok(o) => o,
        Err(_) => return None,
    };
    match output {
        Ok(out) if out.status.success() => Some(String::from_utf8_lossy(&out.stdout).into_owned()),
        _ => None,
    }
}

/// Read-only Git context: `status --porcelain` plus the name-only and
/// bounded unified diff. Only read-only subcommands ever run.
pub struct GitProvider;

impl ContextProvider for GitProvider {
    fn name(&self) -> &'static str {
        PROVIDER_GIT
    }

    fn collect(&self, root: &Path, opts: &ContextOptions) -> Vec<ContextDocument> {
        let cap = opts.byte_cap();
        let mut docs = Vec::new();
        let Some(status) = run_git(root, &["status", "--porcelain"]) else {
            return vec![ContextDocument::unavailable(
                PROVIDER_GIT,
                KIND_GIT_STATUS,
                "git/status",
            )];
        };
        docs.push(build_doc(
            PROVIDER_GIT,
            KIND_GIT_STATUS,
            "git/status",
            status.as_bytes(),
            cap,
            MAX_PREVIEW_BYTES,
            opts,
        ));
        // Changed-surface names: cheap, always useful for triggers.
        if let Some(names) = run_git(root, &["diff", "--name-only", "HEAD"]) {
            docs.push(build_doc(
                PROVIDER_GIT,
                KIND_GIT_DIFF,
                "git/diff-names",
                names.as_bytes(),
                cap,
                MAX_PREVIEW_BYTES,
                opts,
            ));
        }
        // Unified diff body, bounded like every other document.
        if let Some(diff) = run_git(root, &["diff", "HEAD", "--"]) {
            docs.push(build_doc(
                PROVIDER_GIT,
                KIND_GIT_DIFF,
                "git/diff",
                diff.as_bytes(),
                cap,
                MAX_PREVIEW_BYTES,
                opts,
            ));
        }
        docs
    }
}

/// Caller-selected project files, each confined to the project root.
pub struct ProjectFilesProvider;

impl ContextProvider for ProjectFilesProvider {
    fn name(&self) -> &'static str {
        PROVIDER_PROJECT_FILES
    }

    fn collect(&self, root: &Path, opts: &ContextOptions) -> Vec<ContextDocument> {
        if opts.extra_files.is_empty() {
            return vec![ContextDocument::unavailable(
                PROVIDER_PROJECT_FILES,
                KIND_PROJECT_FILE,
                "project-files/selection",
            )];
        }
        opts.extra_files
            .iter()
            .take(opts.doc_cap())
            .map(|f| load_confined_file(root, PROVIDER_PROJECT_FILES, KIND_PROJECT_FILE, f, opts))
            .collect()
    }
}

/// Optional OpenSpec adapter. Reads `openspec/` markdown as plain
/// text into generic documents — no OpenSpec schema is parsed and no
/// OpenSpec types are imported. Missing files yield unavailable
/// markers; nothing is ever mutated.
pub struct OpenspecProvider;

impl OpenspecProvider {
    /// Candidate markdown paths scanned for one change id, in stable
    /// order. Kept as a function so tests assert the exact boundary.
    pub fn change_files(change_id: &str) -> Vec<String> {
        vec![
            format!("openspec/changes/{change_id}/proposal.md"),
            format!("openspec/changes/{change_id}/design.md"),
            format!("openspec/changes/{change_id}/tasks.md"),
            format!("openspec/changes/{change_id}/spec.md"),
        ]
    }

    fn spec_glob_hits(root: &Path, cap: usize) -> Vec<PathBuf> {
        // Bounded manual walk (no glob dependency): top-level specs plus
        // one level of change specs. Deterministic sorted order.
        let mut hits = Vec::new();
        let mut dirs = vec![root.join("openspec/specs")];
        let changes = root.join("openspec/changes");
        if let Ok(entries) = std::fs::read_dir(&changes) {
            let mut names: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            names.sort();
            for dir in names.into_iter().take(8) {
                dirs.push(dir.join("specs"));
            }
        }
        for dir in dirs {
            collect_markdown(&dir, &mut hits, cap);
            if hits.len() >= cap {
                break;
            }
        }
        hits.sort();
        hits.truncate(cap);
        hits
    }
}

fn collect_markdown(dir: &Path, hits: &mut Vec<PathBuf>, cap: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    paths.sort();
    for p in paths {
        if hits.len() >= cap {
            return;
        }
        hits.push(p);
    }
    // One nested level for `specs/<capability>/spec.md` layouts.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    for sub in subdirs.into_iter().take(8) {
        if hits.len() >= cap {
            return;
        }
        collect_markdown(&sub, hits, cap);
    }
}

impl ContextProvider for OpenspecProvider {
    fn name(&self) -> &'static str {
        PROVIDER_OPENSPEC
    }

    fn collect(&self, root: &Path, opts: &ContextOptions) -> Vec<ContextDocument> {
        let mut docs = Vec::new();
        let cap = opts.doc_cap();
        // Change-scoped files when a change id is selected.
        if let Some(change_id) = opts.change_id.clone() {
            for rel in Self::change_files(&change_id) {
                if docs.len() >= cap {
                    break;
                }
                let doc =
                    load_confined_file(root, PROVIDER_OPENSPEC, KIND_OPENSPEC_DOC, &rel, opts);
                docs.push(doc);
            }
        }
        // Capability specs (bounded).
        for path in Self::spec_glob_hits(root, cap.saturating_sub(docs.len()).min(16)) {
            if docs.len() >= cap {
                break;
            }
            let rel = rel_display(root, &path);
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => {
                    docs.push(ContextDocument::unavailable(
                        PROVIDER_OPENSPEC,
                        KIND_OPENSPEC_DOC,
                        &rel,
                    ));
                    continue;
                }
            };
            docs.push(build_doc(
                PROVIDER_OPENSPEC,
                KIND_OPENSPEC_DOC,
                &rel,
                &bytes,
                opts.byte_cap(),
                MAX_PREVIEW_BYTES,
                opts,
            ));
        }
        if docs.is_empty() {
            docs.push(ContextDocument::unavailable(
                PROVIDER_OPENSPEC,
                KIND_OPENSPEC_DOC,
                "openspec/selection",
            ));
        }
        docs
    }
}

/// Context readiness diagnostics for `doctor` (`gate.context.*`).
/// Missing manifests and empty selections stay silent (unconfigured
/// but ok) so reports and exports remain valid without providers.
/// Unknown providers are `Warn`; unreadable manifests are `Warn`.
pub fn context_checks(project_root: &Path) -> Vec<crate::doctor::check::Check> {
    use crate::doctor::check::Check;
    use crate::gate::manifest::{load, manifest_path};
    if manifest_path(project_root).is_none() {
        return Vec::new();
    }
    let manifest = match load(project_root) {
        Ok(Some(m)) => m,
        Ok(None) => return Vec::new(),
        Err(e) => {
            return vec![Check::warn(
                "gate.context.manifest",
                "Gate context manifest is invalid",
                format!("{e}"),
            )
            .with_remediation("Edit gate.toml to fix the parse error.")];
        }
    };
    let selection = selection_from_manifest(&manifest);
    if selection.is_empty() {
        return vec![Check::info(
            "gate.context.configured",
            "No Gate context providers selected",
            "gate planning works without context providers; add [[contexts]] for change context.",
        )];
    }
    let known = [PROVIDER_GIT, PROVIDER_PROJECT_FILES, PROVIDER_OPENSPEC];
    let mut out = Vec::new();
    for name in &selection {
        if !known.contains(&name.as_str()) {
            out.push(
                Check::warn(
                    "gate.context.provider",
                    format!("Unknown context provider `{name}`"),
                    "expected one of: git, project-files, openspec",
                )
                .with_remediation(format!(
                    "Remove `provider = \"{name}\"` from gate.toml or use a supported provider."
                )),
            );
            continue;
        }
        if name == PROVIDER_OPENSPEC && !project_root.join("openspec").is_dir() {
            out.push(
                Check::info(
                    "gate.context.provider",
                    "OpenSpec context selected but no openspec/ directory".to_string(),
                    "context is NOT_APPLICABLE until openspec/ files exist; planning continues without it.",
                )
                .with_remediation(
                    "Create openspec/ files or remove the openspec context declaration.",
                ),
            );
            continue;
        }
        out.push(Check::pass(
            "gate.context.provider",
            format!("Context provider `{name}` is selected"),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::manifest::parse as parse_manifest;

    fn opts() -> ContextOptions {
        ContextOptions::default()
    }

    fn manifest_with(contexts: &[&str]) -> GateManifest {
        let mut text = "version = 1\nprofile = \"minimal\"\n".to_string();
        for c in contexts {
            text.push_str(&format!("[[contexts]]\nprovider = \"{c}\"\n"));
        }
        parse_manifest(&text).unwrap()
    }

    #[test]
    fn empty_selection_yields_valid_empty_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = collect_context(tmp.path(), &[], &opts());
        assert!(bundle.docs.is_empty());
        assert!(bundle.diagnostics.is_empty());
        assert_eq!(bundle.available_count(), 0);
    }

    #[test]
    fn git_only_project_needs_no_openspec_files() {
        let tmp = tempfile::tempdir().unwrap();
        // No openspec/ dir at all; git provider only.
        let bundle = collect_context(tmp.path(), &[PROVIDER_GIT.to_string()], &opts());
        // Outside a worktree the doc is unavailable — but collection
        // succeeds and claims nothing.
        assert_eq!(bundle.docs.len(), 1);
        assert!(!bundle.docs[0].available);
        assert_eq!(bundle.docs[0].safe_preview(), UNAVAILABLE_PREVIEW);
        assert!(bundle.render().contains("NOT_APPLICABLE"));
    }

    #[test]
    fn git_provider_inside_worktree_is_available() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status();
        if status.is_err() || !status.unwrap().success() {
            return;
        }
        std::fs::write(root.join("a.txt"), "hello\n").unwrap();
        let bundle = collect_context(root, &[PROVIDER_GIT.to_string()], &opts());
        assert!(!bundle.docs.is_empty());
        assert!(bundle.docs.iter().any(|d| d.available));
        let status_doc = bundle
            .docs
            .iter()
            .find(|d| d.kind == KIND_GIT_STATUS)
            .unwrap();
        assert!(status_doc.digest.starts_with("sha256:"));
        assert!(status_doc.preview.as_deref().unwrap().contains("a.txt"));
    }

    #[test]
    fn git_provider_never_mutates_the_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status();
        if status.is_err() || !status.unwrap().success() {
            return;
        }
        std::fs::write(root.join("a.txt"), "hello\n").unwrap();
        let before = run_git(root, &["status", "--porcelain"]).unwrap();
        let _ = collect_context(root, &[PROVIDER_GIT.to_string()], &opts());
        // Providers run read-only git subcommands; the tree is untouched.
        let after = run_git(root, &["status", "--porcelain"]).unwrap();
        assert_eq!(before, after);
        // The tracked file itself is untouched.
        assert!(root.join("a.txt").exists());
    }

    #[test]
    fn openspec_project_loads_generic_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("openspec/changes/demo")).unwrap();
        std::fs::write(root.join("openspec/changes/demo/proposal.md"), "# Why\n").unwrap();
        std::fs::write(root.join("openspec/changes/demo/design.md"), "# Design\n").unwrap();
        let mut o = opts();
        o.change_id = Some("demo".to_string());
        let bundle = collect_context(root, &[PROVIDER_OPENSPEC.to_string()], &o);
        let available: Vec<_> = bundle.docs.iter().filter(|d| d.available).collect();
        assert!(available.len() >= 2, "got: {:?}", bundle.docs);
        // Generic output only: kinds are plain strings, no OpenSpec types.
        for d in &available {
            assert_eq!(d.kind, KIND_OPENSPEC_DOC);
            assert_eq!(d.provider, PROVIDER_OPENSPEC);
        }
        assert!(bundle.render().contains("available"));
    }

    #[test]
    fn missing_openspec_files_are_unavailable_not_success() {
        let tmp = tempfile::tempdir().unwrap();
        let mut o = opts();
        o.change_id = Some("no-such-change".to_string());
        let bundle = collect_context(tmp.path(), &[PROVIDER_OPENSPEC.to_string()], &o);
        assert!(!bundle.docs.is_empty());
        // Change-scoped files are unavailable; nothing claims requirements.
        let change_docs: Vec<_> = bundle
            .docs
            .iter()
            .filter(|d| d.path.contains("no-such-change"))
            .collect();
        assert!(!change_docs.is_empty());
        assert!(change_docs.iter().all(|d| !d.available));
        for d in &change_docs {
            assert_eq!(d.safe_preview(), UNAVAILABLE_PREVIEW);
        }
    }

    #[test]
    fn context_path_escape_fails_safely() {
        let tmp = tempfile::tempdir().unwrap();
        let mut o = opts();
        o.extra_files = vec!["../../etc/passwd".to_string(), "/etc/passwd".to_string()];
        let bundle = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        assert_eq!(bundle.docs.len(), 2);
        assert!(bundle.docs.iter().all(|d| !d.available));
    }

    #[test]
    fn oversized_document_truncates_and_never_loads_unbounded() {
        let tmp = tempfile::tempdir().unwrap();
        let big = vec![b'x'; MAX_CONTEXT_BYTES + 4096];
        std::fs::write(tmp.path().join("big.md"), &big).unwrap();
        let mut o = opts();
        o.extra_files = vec!["big.md".to_string()];
        let bundle = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        assert_eq!(bundle.docs.len(), 1);
        let doc = &bundle.docs[0];
        assert!(doc.available);
        assert!(doc.truncated);
        assert!((doc.byte_size as usize) <= MAX_CONTEXT_BYTES);
        assert!((doc.preview.as_deref().unwrap().len()) <= MAX_PREVIEW_BYTES + 8);
    }

    #[test]
    fn project_files_do_not_mutate_sources() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("keep.md"), "original\n").unwrap();
        let mtime_before = std::fs::metadata(tmp.path().join("keep.md"))
            .unwrap()
            .modified()
            .unwrap();
        let content_before = std::fs::read(tmp.path().join("keep.md")).unwrap();
        let mut o = opts();
        o.extra_files = vec!["keep.md".to_string()];
        let _ = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        assert_eq!(
            std::fs::read(tmp.path().join("keep.md")).unwrap(),
            content_before
        );
        assert_eq!(
            std::fs::metadata(tmp.path().join("keep.md"))
                .unwrap()
                .modified()
                .unwrap(),
            mtime_before
        );
    }

    #[test]
    fn secrets_are_redacted_in_previews() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("leak.md"), "api_key=supersecret123\n").unwrap();
        let mut o = opts();
        o.extra_files = vec!["leak.md".to_string()];
        o.sensitive = vec!["supersecret123".to_string()];
        let bundle = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        let preview = bundle.docs[0].preview.clone().unwrap();
        assert!(!preview.contains("supersecret123"));
        assert!(preview.contains("[REDACTED]"));
    }

    #[test]
    fn digests_are_stable_and_prefixed() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.md"), "same\n").unwrap();
        let mut o = opts();
        o.extra_files = vec!["a.md".to_string()];
        let sel = vec![PROVIDER_PROJECT_FILES.to_string()];
        let a = collect_context(tmp.path(), &sel, &o);
        let b = collect_context(tmp.path(), &sel, &o);
        assert_eq!(a.docs[0].digest, b.docs[0].digest);
        assert!(a.docs[0].digest.starts_with("sha256:"));
    }

    #[test]
    fn unknown_provider_yields_unavailable_and_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = collect_context(tmp.path(), &["jira-pro".to_string()], &opts());
        assert_eq!(bundle.docs.len(), 1);
        assert!(!bundle.docs[0].available);
        assert!(bundle
            .diagnostics
            .iter()
            .any(|d| d.contains("unknown context provider")));
    }

    #[test]
    fn registry_isolates_provider_outcomes() {
        struct Fail;
        impl ContextProvider for Fail {
            fn name(&self) -> &'static str {
                "fail"
            }
            fn collect(&self, _root: &Path, _opts: &ContextOptions) -> Vec<ContextDocument> {
                vec![ContextDocument::unavailable("fail", "x", "fail/y")]
            }
        }
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(Fail));
        reg.register(Box::new(ProjectFilesProvider));
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("ok.md"), "ok\n").unwrap();
        let mut o = opts();
        o.extra_files = vec!["ok.md".to_string()];
        let bundle = collect_with_registry(
            tmp.path(),
            &["fail".to_string(), PROVIDER_PROJECT_FILES.to_string()],
            &o,
            &reg,
        );
        // The failing provider's marker does not abort the second one.
        assert_eq!(bundle.docs.len(), 2);
        assert!(!bundle.docs[0].available);
        assert!(bundle.docs[1].available);
    }

    #[test]
    fn document_cap_is_enforced() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..6 {
            std::fs::write(tmp.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut o = opts();
        o.extra_files = (0..6).map(|i| format!("f{i}.md")).collect();
        o.max_docs = Some(2);
        let bundle = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        assert!(bundle.docs.len() <= 2);
    }

    #[test]
    fn selection_from_manifest_dedups_and_orders() {
        let m = manifest_with(&["openspec", "git", "openspec"]);
        assert_eq!(
            selection_from_manifest(&m),
            vec!["openspec".to_string(), "git".to_string()]
        );
        let empty = manifest_with(&[]);
        assert!(selection_from_manifest(&empty).is_empty());
    }

    #[test]
    fn context_checks_silent_without_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(context_checks(tmp.path()).is_empty());
    }

    #[test]
    fn context_checks_flag_unknown_providers() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("gate.toml"),
            "version = 1\nprofile = \"minimal\"\n[[contexts]]\nprovider = \"jira-pro\"\n",
        )
        .unwrap();
        let checks = context_checks(tmp.path());
        assert!(checks.iter().any(|c| c.id == "gate.context.provider"));
        assert!(checks
            .iter()
            .all(|c| c.status != crate::doctor::check::Status::Fail));
    }

    #[test]
    fn context_checks_info_when_no_openspec_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("gate.toml"),
            "version = 1\nprofile = \"minimal\"\n[[contexts]]\nprovider = \"openspec\"\n",
        )
        .unwrap();
        let checks = context_checks(tmp.path());
        // No openspec/ dir: honest INFO, never a false PASS-with-content.
        assert!(checks
            .iter()
            .any(|c| c.status == crate::doctor::check::Status::Info));
    }

    #[test]
    fn openspec_change_file_boundary_is_stable() {
        let files = OpenspecProvider::change_files("demo");
        assert_eq!(
            files,
            vec![
                "openspec/changes/demo/proposal.md".to_string(),
                "openspec/changes/demo/design.md".to_string(),
                "openspec/changes/demo/tasks.md".to_string(),
                "openspec/changes/demo/spec.md".to_string(),
            ]
        );
    }

    #[test]
    fn previews_stay_utf8_safe() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("uni.md"), "汉".repeat(5000)).unwrap();
        let mut o = opts();
        o.extra_files = vec!["uni.md".to_string()];
        let bundle = collect_context(tmp.path(), &[PROVIDER_PROJECT_FILES.to_string()], &o);
        let preview = bundle.docs[0].preview.clone().unwrap();
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
    }
}
