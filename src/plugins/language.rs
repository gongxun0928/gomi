use super::{ArtifactSpec, LanguagePlugin};

macro_rules! explain_triple {
    ($what:expr, $del:expr, $reb:expr) => {
        ($what.to_string(), $del.to_string(), $reb.to_string())
    };
}

struct RustLang;
struct NodeLang;
struct CppLang;
struct PythonLang;
struct GoLang;
struct JavaLang;
struct SwiftLang;

const RUST_ART: &[ArtifactSpec] = &[ArtifactSpec {
    id: "target",
    exact_names: &["target"],
    prefixes: &[],
}];
const NODE_ART: &[ArtifactSpec] = &[ArtifactSpec {
    id: "node_modules",
    exact_names: &["node_modules"],
    prefixes: &[],
}];
const CPP_ART: &[ArtifactSpec] = &[ArtifactSpec {
    id: "build",
    exact_names: &["build"],
    prefixes: &["cmake-build-"],
}];
const PY_ART: &[ArtifactSpec] = &[
    ArtifactSpec {
        id: "pycache",
        exact_names: &["__pycache__"],
        prefixes: &[],
    },
    ArtifactSpec {
        id: "venv",
        exact_names: &[".venv"],
        prefixes: &[],
    },
];
const GO_ART: &[ArtifactSpec] = &[ArtifactSpec {
    id: "vendor",
    exact_names: &["vendor"],
    prefixes: &[],
}];
const JAVA_ART: &[ArtifactSpec] = &[
    ArtifactSpec {
        id: "target",
        exact_names: &["target"],
        prefixes: &[],
    },
    ArtifactSpec {
        id: "build",
        exact_names: &["build"],
        prefixes: &[],
    },
];
const SWIFT_ART: &[ArtifactSpec] = &[ArtifactSpec {
    id: "build",
    exact_names: &[".build"],
    prefixes: &[],
}];

impl LanguagePlugin for RustLang {
    fn id(&self) -> &'static str {
        "rust"
    }
    fn display_name(&self) -> &'static str {
        "Rust"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        RUST_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "Rust build output (target/).",
            "Next cargo build will recompile. Source is untouched.",
            "cargo build at the repo root."
        )
    }
}

impl LanguagePlugin for NodeLang {
    fn id(&self) -> &'static str {
        "node"
    }
    fn display_name(&self) -> &'static str {
        "Node"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["package.json"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        NODE_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "Node dependencies (node_modules/).",
            "Dependencies must be reinstalled.",
            "npm/pnpm/yarn/bun install."
        )
    }
}

impl LanguagePlugin for CppLang {
    fn id(&self) -> &'static str {
        "cpp"
    }
    fn display_name(&self) -> &'static str {
        "C/C++"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["CMakeLists.txt", "Makefile", "compile_commands.json"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        CPP_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "C/C++ build directory.",
            "Requires cmake/make again.",
            "Reconfigure and compile."
        )
    }
}

impl LanguagePlugin for PythonLang {
    fn id(&self) -> &'static str {
        "python"
    }
    fn display_name(&self) -> &'static str {
        "Python"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["pyproject.toml", "requirements.txt"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        PY_ART
    }
    fn explain(&self, artifact_id: &str) -> (String, String, String) {
        if artifact_id == "venv" {
            explain_triple!(
                "Python venv (.venv/). Rebuildable; reinstall can be slow.",
                "The current interpreter env is gone.",
                "python -m venv .venv && pip install -r requirements.txt"
            )
        } else {
            explain_triple!(
                "Python bytecode cache.",
                "Recreated at runtime.",
                "No manual rebuild."
            )
        }
    }
}

impl LanguagePlugin for GoLang {
    fn id(&self) -> &'static str {
        "go"
    }
    fn display_name(&self) -> &'static str {
        "Go"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["go.mod"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        GO_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "Go vendor/ copy of dependencies.",
            "Regenerate with go mod vendor.",
            "go mod vendor"
        )
    }
}

impl LanguagePlugin for JavaLang {
    fn id(&self) -> &'static str {
        "java"
    }
    fn display_name(&self) -> &'static str {
        "Java"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        JAVA_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "Maven/Gradle build output.",
            "Requires a rebuild.",
            "mvn compile or gradle build."
        )
    }
}

impl LanguagePlugin for SwiftLang {
    fn id(&self) -> &'static str {
        "swift"
    }
    fn display_name(&self) -> &'static str {
        "Swift"
    }
    fn project_markers(&self) -> &'static [&'static str] {
        &["Package.swift"]
    }
    fn artifacts(&self) -> &'static [ArtifactSpec] {
        SWIFT_ART
    }
    fn explain(&self, _artifact_id: &str) -> (String, String, String) {
        explain_triple!(
            "SwiftPM build directory (.build/).",
            "Requires swift build again.",
            "swift build"
        )
    }
}

static RUST: RustLang = RustLang;
static NODE: NodeLang = NodeLang;
static CPP: CppLang = CppLang;
static PYTHON: PythonLang = PythonLang;
static GO: GoLang = GoLang;
static JAVA: JavaLang = JavaLang;
static SWIFT: SwiftLang = SwiftLang;

pub fn all() -> Vec<&'static dyn LanguagePlugin> {
    vec![&RUST, &NODE, &CPP, &PYTHON, &GO, &JAVA, &SWIFT]
}

pub fn claim(
    artifact_dir: &std::path::Path,
    stop: &std::path::Path,
) -> Option<(&'static dyn LanguagePlugin, &'static ArtifactSpec, usize)> {
    let name = artifact_dir.file_name()?.to_str()?;
    let mut best: Option<(&'static dyn LanguagePlugin, &'static ArtifactSpec, usize)> = None;
    for lang in all() {
        for art in lang.artifacts() {
            if !art.matches(name) {
                continue;
            }
            if let Some(dist) = marker_distance(artifact_dir, stop, lang.project_markers()) {
                let better = match best {
                    None => true,
                    Some((_, _, d)) => dist < d,
                };
                if better {
                    best = Some((lang, art, dist));
                }
            }
        }
    }
    best
}

fn marker_distance(start: &std::path::Path, stop: &std::path::Path, markers: &[&str]) -> Option<usize> {
    let git_stop = crate::gitx::nearest_git_root(start, stop).unwrap_or_else(|| stop.to_path_buf());
    let mut cur = start.parent()?.to_path_buf();
    let mut dist = 1usize;
    loop {
        if markers.iter().any(|m| cur.join(m).is_file()) {
            return Some(dist);
        }
        if cur == git_stop || cur == stop {
            return None;
        }
        if !cur.pop() {
            return None;
        }
        dist += 1;
        if dist > 24 {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("gomi-lang-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn target_without_cargo_toml_is_not_rust() {
        let root = tmp("nomarker");
        let target = root.join("target");
        fs::create_dir_all(&target).unwrap();
        assert!(claim(&target, &root).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn target_with_cargo_toml_is_rust() {
        let root = tmp("marked");
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        let (lang, art, _) = claim(&root.join("target"), &root).expect("claim");
        assert_eq!(lang.id(), "rust");
        assert_eq!(art.id, "target");
        let _ = fs::remove_dir_all(&root);
    }
}
