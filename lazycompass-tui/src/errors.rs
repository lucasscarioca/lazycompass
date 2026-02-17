use super::*;
pub(crate) fn is_network_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        message.contains("unable to connect")
            || message.contains("failed to connect")
            || message.contains("server selection")
            || message.contains("network")
            || message.contains("timed out")
            || message.contains("timeout")
    })
}

pub(crate) fn is_network_error_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("unable to connect")
        || message.contains("failed to connect")
        || message.contains("server selection")
        || message.contains("network")
        || message.contains("timed out")
        || message.contains("timeout")
}

pub(crate) fn format_error_message(message: &str, is_network: bool) -> String {
    let mut output = redact_sensitive_text(message);
    if is_network {
        output.push_str(" (network error: retry read-only operations)");
    }
    output
}
