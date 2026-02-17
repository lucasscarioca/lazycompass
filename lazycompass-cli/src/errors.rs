use anyhow::Error;
use lazycompass_core::redact_sensitive_text;
use lazycompass_storage::StorageSnapshot;

const EXIT_USER: i32 = 1;
const EXIT_CONFIG: i32 = 2;
const EXIT_NETWORK: i32 = 3;

pub(crate) fn report_error(error: &Error) {
    eprintln!("error: {}", redact_sensitive_text(&error.to_string()));
    for cause in error.chain().skip(1) {
        eprintln!("caused by: {}", redact_sensitive_text(&cause.to_string()));
    }
    if network_message_matches(error) {
        eprintln!("note: network errors can be transient; retry read-only operations");
    }
}

pub(crate) fn exit_code(error: &Error) -> i32 {
    if error_chain_has::<std::io::Error>(error) || config_message_matches(error) {
        return EXIT_CONFIG;
    }
    if network_message_matches(error) {
        return EXIT_NETWORK;
    }
    EXIT_USER
}

pub(crate) fn report_warnings(storage: &StorageSnapshot) {
    for warning in &storage.warnings {
        eprintln!("warning: {}", redact_sensitive_text(warning));
    }
}

fn config_message_matches(error: &Error) -> bool {
    error_chain_matches(error, |message| {
        message.contains("config") || message.contains("toml")
    })
}

fn network_message_matches(error: &Error) -> bool {
    error_chain_matches(error, |message| {
        message.contains("unable to connect")
            || message.contains("failed to connect")
            || message.contains("server selection")
            || message.contains("network")
            || message.contains("timed out")
            || message.contains("timeout")
    })
}

fn error_chain_has<T: std::error::Error + 'static>(error: &Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<T>().is_some())
}

fn error_chain_matches(error: &Error, predicate: impl Fn(&str) -> bool) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        predicate(&message)
    })
}
