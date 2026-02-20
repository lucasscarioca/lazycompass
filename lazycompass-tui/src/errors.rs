use super::*;
fn is_query_timeout_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("maxtimemsexpired")
        || message.contains("max time ms")
        || message.contains("max execution time")
        || message.contains("execution time limit")
        || message.contains("exceeded time limit")
}

fn is_query_timeout_error(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| is_query_timeout_message(&cause.to_string()))
}

fn root_cause_message(error: &anyhow::Error) -> Option<String> {
    let mut last = None;
    for cause in error.chain() {
        last = Some(cause.to_string());
    }
    last
}

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

pub(crate) fn format_error(error: &anyhow::Error) -> String {
    let primary = redact_sensitive_text(&error.to_string());
    let cause = root_cause_message(error).map(|message| redact_sensitive_text(&message));
    let has_distinct_cause = cause
        .as_ref()
        .map(|value| {
            !primary
                .to_ascii_lowercase()
                .contains(&value.to_ascii_lowercase())
        })
        .unwrap_or(false);

    if is_query_timeout_error(error) {
        let mut output =
            "query timeout (maxTimeMS): increase [timeouts].query_ms or narrow filter/sort"
                .to_string();
        output.push_str("; ");
        output.push_str(&primary);
        if let Some(cause) = cause
            && has_distinct_cause
        {
            output.push_str(" (cause: ");
            output.push_str(&cause);
            output.push(')');
        }
        return output;
    }

    let mut output = primary;
    if let Some(cause) = cause
        && has_distinct_cause
    {
        output.push_str(" (cause: ");
        output.push_str(&cause);
        output.push(')');
    }
    format_error_message(&output, is_network_error(error))
}

pub(crate) fn format_error_message(message: &str, is_network: bool) -> String {
    let mut output = redact_sensitive_text(message);
    if is_network {
        output.push_str(" (network error: retry read-only operations)");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::format_error;

    #[test]
    fn format_error_highlights_query_timeout() {
        let error = anyhow::anyhow!("MaxTimeMSExpired: operation exceeded time limit")
            .context("failed to run find on app.users");
        let message = format_error(&error);

        assert!(message.contains("query timeout (maxTimeMS)"));
        assert!(message.contains("[timeouts].query_ms"));
    }

    #[test]
    fn format_error_includes_distinct_root_cause() {
        let error = anyhow::anyhow!("server selection timeout")
            .context("failed to load documents from app.users");
        let message = format_error(&error);

        assert!(message.contains("cause: server selection timeout"));
        assert!(message.contains("network error"));
    }
}
