mod app_info;
mod error;
mod wire;

pub use app_info::{app_info, AppInfo, AppRuntime, BuildProfile, GET_APP_INFO_COMMAND};
pub use error::AppError;
pub use wire::typescript_bindings;
