pub(crate) fn ctrlc_handler() -> tokio::task::JoinHandle<()> {
    // on ctrl-c,  dialoguer/console will clean up the rest of its
    // formatting, but the cursor will remain hidden unless we
    // explicitly go in and show it again
    // See: https://github.com/console-rs/dialoguer/issues/294
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();

        let term = console::Term::stdout();
        // Ignore the error here if there is any, this is best effort
        let _ = term.show_cursor();

        // Immediately re-exit the process with the same
        // exit code the unhandled ctrl-c would have used
        let exitstatus = if cfg!(windows) {
            0xc000013a_u32 as i32
        } else {
            130
        };
        std::process::exit(exitstatus);
    })
}

pub(crate) fn theme() -> dialoguer::theme::ColorfulTheme {
    dialoguer::theme::ColorfulTheme {
        checked_item_prefix: console::style("  [x]".to_string()).for_stderr().green(),
        unchecked_item_prefix: console::style("  [ ]".to_string()).for_stderr().dim(),
        active_item_style: console::Style::new().for_stderr().cyan().bold(),
        ..dialoguer::theme::ColorfulTheme::default()
    }
}
