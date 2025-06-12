# Version 0.3.3 (2024-07-05)

This release adds the tls-native-roots feature, which enables the same
underlying feature in reqwest, making us also support native certificate chains.


# Version 0.3.2 (2024-06-12)

This release changes how reqwest is used, inheriting it from a dependency rather than specifying it directly. It also updates several dependencies.

# Version 0.3.1 (2024-04-16)

Updates dependencies, relaxing the ranges on axoasset and reqwest. Also improves an error message for mocked request errors.

# Version 0.3.0 (2024-02-16)

Updates dependencies, including a breaking change to miette.

# Version 0.2.3 (2024-02-08)

Fix to STAGE_INTO_THE_ABYSS mode.


# Version 0.2.2 (2023-12-11)

Add a STAGE_INTO_THE_ABYSS=1 env-var to toggle the client into "staging" mode,
where it reads and write's axo releases staging servers instead of production.
This is for creating realistic test data.

# Version 0.2.1 (2023-12-11)

* Fix to an incorrect URL for the release data endpoint


# Version 0.2.0 (2023-12-11)

* Properly implemented the functions calls to get release data for packages, for oranda
  (and other things') usage!


# Version 0.1.4 (2023-11-22)

* updated API domains to production servers
* properly made a client_lib dependency optional


# Version 0.1.3 (2023-11-21)

* There is now a maximum limit of 10 connections from gazenot at a time
* There is now a retry/backoff system for server 500 errors (3 tries, delays: 1s, 2s)
* The API domains that gazenot accesses can now be overridden programmatically or with env-vars


# Version 0.1.2 (2023-11-20)

Made bulk file upload API serial as a quick-n-dirty solution to too many connections.

See https://github.com/axodotdev/gazenot/issues/10 for details.


# Version 0.1.1 (2023-11-17)

Increased timeout for file uploads to 3 minutes.


# Version 0.1.0 (2023-11-17)

Initial Release!

Some functionality still missing, but core functionality for cargo-dist's uses should be fully operational!
