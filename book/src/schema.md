# JSON Schema

Many cargo-dist commands when run with `--output-format=json` will output to stdout a format we call "dist-manifest.json". This contains:

* Top-level facts about the Announcement (tag, announcement title, etc)
* Info about the Apps being Released as part of the Announcement ("releases")
* Info about the Artifacts included in the Announcement ("announcements")

As a matter of forward-compat and back-compat, basically every field in the format should be treated as optional (which the schema reflects).

Here is the JSON Schema which will *hopefully* be stable going forward.

```json
{{#include ../../cargo-dist-schema/src/snapshots/cargo_dist_schema__emit.snap:5:}}
```