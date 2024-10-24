# Updating

Just

`dist init`

That's it!

Rerun init as much as you want, it should always preserve your old settings, and should never break anything (if it does, it's a bug!).

If you have a project with dist setup on it, and want to upgrade to a new version, all you should ever need to do is locally install the new version of dist and run `dist init`.

If you're simply adjusting your dist config, you should also rerun `dist init` to potentially get informed of new features/constraints your change runs into. It also ensures that things like your ci.yml are updated to reflect your new config. Basically every other dist command should error out if you *have* to rerun init, so you won't get far if you don't.

We recommend running without `-y` for reruns, because this is the chance dist has to tell you about new features, or additional configuration that pairs with any adjustments you may have made. It will take that chance to ask you if you want to enable the feature or change the default value.

In general the init command is designed to do incremental updates to your installation, and "first setup" is just a special case of this, where every incremental update is applicable.

The command usually uses the absence of a setting in your config to determine if a feature has been setup before. As such, even though dist *generally* has default values for every piece of config, init will aggressively write the default back to your config to let future invocations know they don't need to ask about it.

Which also means if you missed a prompt or want to reconfigure a feature, deleting the relevant setting from your config and rerunning `init` should work.

There are two settings that init will always prompt you for:

* what platforms do you want to build for
* what installers do you want to have

So if you ever want to add a new platform or installer, rerunning `dist init` is a great way to do that -- and then it can ask followup questions if you turn on a new installer!
