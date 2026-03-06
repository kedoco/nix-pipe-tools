pub mod hash;
pub mod fileident;
pub mod human;

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), env!("GIT_VERSION_SUFFIX"));
