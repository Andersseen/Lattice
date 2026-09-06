use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppRuntime {
    Tauri,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildProfile {
    Debug,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub build_profile: BuildProfile,
    pub runtime: AppRuntime,
    pub target: &'static str,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Lattice",
        version: env!("CARGO_PKG_VERSION"),
        build_profile: current_build_profile(),
        runtime: AppRuntime::Tauri,
        target: current_target(),
    }
}

fn current_build_profile() -> BuildProfile {
    if cfg!(debug_assertions) {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    }
}

fn current_target() -> &'static str {
    std::env::consts::OS
}

#[cfg(test)]
mod tests {
    use super::{app_info, AppRuntime};

    #[test]
    fn exposes_foundation_app_info() {
        let info = app_info();

        assert_eq!(info.name, "Lattice");
        assert_eq!(info.runtime, AppRuntime::Tauri);
        assert!(!info.version.is_empty());
        assert!(!info.target.is_empty());
    }
}
