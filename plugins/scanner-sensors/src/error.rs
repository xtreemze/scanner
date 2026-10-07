use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),

    #[cfg(desktop)]
    #[error("scanner sensor plugin is unavailable on this platform")]
    UnsupportedPlatform,
}

pub type Result<T> = std::result::Result<T, Error>;
