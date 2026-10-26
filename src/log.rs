//! Best-effort terminal writes: a failure is logged through `tracing`, never dropped silently.

/// Logs the error of a `Result` whose failure the caller can live with.
pub(crate) trait OrWarn {
    /// `what` names the work in the log line, before the error.
    fn or_warn(self, what: &str);
}

impl<T, E: std::fmt::Display> OrWarn for Result<T, E> {
    fn or_warn(self, what: &str) {
        if let Err(e) = self {
            tracing::warn!("{what} failed: {e:#}");
        }
    }
}
