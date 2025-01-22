pub(crate) fn theme() -> dialoguer::theme::ColorfulTheme {
    dialoguer::theme::ColorfulTheme {
        checked_item_prefix: console::style("  [x]".to_string()).for_stderr().green(),
        unchecked_item_prefix: console::style("  [ ]".to_string()).for_stderr().dim(),
        active_item_style: console::Style::new().for_stderr().cyan().bold(),
        ..dialoguer::theme::ColorfulTheme::default()
    }
}
