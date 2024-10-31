# Other Installation Methods

dist projects can also theoretically be installed with the following, through no active effort of our own:

* [cargo-install][] (just [cargo publish][] like normal)
* [cargo-binstall][] (the URL schema we use for Github Releases is auto-detected)

In the future we might [support displaying these kinds of install methods][issue-info-install].

Note that cargo-install is just building from the uploaded source with the --release profile, and so if you're relying on dist or unpublished files for some key behaviours, this may cause problems. [It also disrespects your lockfile unless you pass --locked][install-locked]. You can more closely approximate dist's build with:

```sh
cargo install --locked
```

Although that's still missing things like [Windows crt-static workarounds][crt-static] and the "dist" profile.


