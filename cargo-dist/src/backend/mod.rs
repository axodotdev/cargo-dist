//! The backend of cargo-dist -- things it outputs

pub mod ci;
pub mod installer;

/// Name of the installer.sh template
pub const TEMPLATE_INSTALLER_SH: &str = "installer.sh";
/// Name of the installer.ps1 template
pub const TEMPLATE_INSTALLER_PS: &str = "installer.ps1";

/// Load+parse templates for various things (ideally done only once and then reused)
pub fn make_template_env() -> minijinja::Environment<'static> {
    let mut env = minijinja::Environment::new();
    env.set_debug(true);

    fn jinja_error(details: String) -> std::result::Result<String, minijinja::Error> {
        Err(minijinja::Error::new(
            minijinja::ErrorKind::EvalBlock,
            details,
        ))
    }

    env.add_function("error", jinja_error);

    env.add_template(
        TEMPLATE_INSTALLER_SH,
        include_str!("templates/installer.sh.j2"),
    )
    .expect("failed to load installer.sh template from binary");
    env.add_template(
        TEMPLATE_INSTALLER_PS,
        include_str!("templates/installer.ps1.j2"),
    )
    .expect("failed to load installer.sh template from binary");
    env
}
