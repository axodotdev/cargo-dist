use crate::{config::Config, errors::DistResult, PROFILE_DIST};
use axoasset::toml_edit;

pub fn init_dist_profile(
    _cfg: &Config,
    workspace_toml: &mut toml_edit::DocumentMut,
) -> DistResult<bool> {
    let profiles = workspace_toml["profile"].or_insert(toml_edit::table());
    if let Some(t) = profiles.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_profile = &mut profiles[PROFILE_DIST];
    if !dist_profile.is_none() {
        return Ok(false);
    }
    let mut new_profile = toml_edit::table();
    {
        // For some detailed discussion, see: https://github.com/axodotdev/cargo-dist/issues/118
        let new_profile = new_profile.as_table_mut().unwrap();
        // We're building for release, so this is a good base!
        new_profile.insert("inherits", toml_edit::value("release"));
        // We're building for SUPER DUPER release, so lto is a good idea to enable!
        //
        // There's a decent argument for lto=true (aka "fat") here but the cost-benefit
        // is a bit complex. Fat LTO can be way more expensive to compute (to the extent
        // that enormous applications like chromium can become unbuildable), but definitely
        // eeks out a bit more from your binaries.
        //
        // In principle dist is targeting True Shippable Binaries and so it's
        // worth it to go nuts getting every last drop out of your binaries... but a lot
        // of people are going to build binaries that might never even be used, so really
        // we're just burning a bunch of CI time for nothing.
        //
        // The user has the freedom to crank this up higher (and/or set codegen-units=1)
        // if they think it's worth it, but we otherwise probably shouldn't set the planet
        // on fire just because Number Theoretically Go Up.
        new_profile.insert("lto", toml_edit::value("thin"));
        new_profile
            .decor_mut()
            .set_prefix("\n# The profile that 'dist' will build with\n")
    }
    dist_profile.or_insert(new_profile);

    Ok(true)
}
