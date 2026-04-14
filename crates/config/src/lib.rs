mod paths;
mod persistence;

pub use paths::config_dir;
pub use persistence::{load_state, save_state};
