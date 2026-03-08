pub mod output;
pub mod query;
pub mod types;

#[cfg(not(target_os = "linux"))]
pub mod lsof;

#[cfg(target_os = "linux")]
pub mod procfs;
