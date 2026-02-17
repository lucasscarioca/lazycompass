use super::*;
pub(crate) fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("unable to enable raw mode")?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen, Hide).context("unable to enter alternate screen")?;
    let backend = CrosstermBackend::new(output);
    Terminal::new(backend).context("unable to start terminal")
}

pub(crate) fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("unable to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)
        .context("unable to leave alternate screen")?;
    terminal.show_cursor().context("unable to restore cursor")?;
    Ok(())
}

pub(crate) fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("unable to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)
        .context("unable to leave alternate screen")?;
    terminal.show_cursor().context("unable to show cursor")?;
    Ok(())
}

pub(crate) fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    enable_raw_mode().context("unable to enable raw mode")?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, Hide)
        .context("unable to enter alternate screen")?;
    terminal.clear().context("unable to clear terminal")?;
    Ok(())
}
