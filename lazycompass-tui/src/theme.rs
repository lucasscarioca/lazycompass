use super::*;
#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    text: Color,
    accent: Color,
    border: Color,
    selection_fg: Color,
    selection_bg: Color,
    warning: Color,
    error: Color,
}

impl Theme {
    pub(crate) fn text_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub(crate) fn title_style(self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub(crate) fn border_style(self) -> Style {
        Style::default().fg(self.border)
    }

    pub(crate) fn selection_style(self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub(crate) fn warning_style(self) -> Style {
        Style::default().fg(self.warning)
    }

    pub(crate) fn error_style(self) -> Style {
        Style::default().fg(self.error)
    }
}

const THEME_CLASSIC: Theme = Theme {
    text: Color::Gray,
    accent: Color::Cyan,
    border: Color::DarkGray,
    selection_fg: Color::Black,
    selection_bg: Color::Cyan,
    warning: Color::Yellow,
    error: Color::Red,
};

const THEME_EMBER: Theme = Theme {
    text: Color::White,
    accent: Color::LightRed,
    border: Color::Red,
    selection_fg: Color::Black,
    selection_bg: Color::LightRed,
    warning: Color::LightYellow,
    error: Color::LightRed,
};
pub(crate) fn resolve_theme(config: &Config) -> (Theme, Option<String>) {
    let name = config.theme.name.as_deref().unwrap_or_default();
    if name.trim().is_empty() {
        return (THEME_CLASSIC, None);
    }
    match theme_by_name(name) {
        Some(theme) => (theme, None),
        None => (
            THEME_CLASSIC,
            Some(format!("unknown theme '{name}', using classic")),
        ),
    }
}

pub(crate) fn theme_by_name(name: &str) -> Option<Theme> {
    match name.trim().to_ascii_lowercase().as_str() {
        "classic" | "default" => Some(THEME_CLASSIC),
        "ember" => Some(THEME_EMBER),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub(crate) fn resolve_theme_warns_on_unknown() {
        let config = Config {
            connections: Vec::new(),
            theme: lazycompass_core::ThemeConfig {
                name: Some("mystery".to_string()),
            },
            logging: lazycompass_core::LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: None,
            timeouts: lazycompass_core::TimeoutConfig::default(),
        };
        let (theme, warning) = resolve_theme(&config);
        assert!(warning.is_some());
        assert_eq!(theme.border, THEME_CLASSIC.border);
    }

    #[test]
    pub(crate) fn resolve_theme_uses_ember() {
        let config = Config {
            connections: Vec::new(),
            theme: lazycompass_core::ThemeConfig {
                name: Some("ember".to_string()),
            },
            logging: lazycompass_core::LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: None,
            timeouts: lazycompass_core::TimeoutConfig::default(),
        };
        let (theme, warning) = resolve_theme(&config);
        assert!(warning.is_none());
        assert_eq!(theme.accent, THEME_EMBER.accent);
    }
}
