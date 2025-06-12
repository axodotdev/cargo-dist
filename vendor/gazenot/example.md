
Shipping applications with the Abyss is a 5 step process:

```no_run
use camino::Utf8PathBuf;
use gazenot::{AnnouncementKey, Gazenot, ReleaseKey};

#[tokio::main]
async fn main() -> Result<(), miette::Report> {
    // Data we want to submit to Abyss
    let (source_host, owner) = ("github", "axodotdev");
    let apps = vec!["app1", "app2"];
    let release_version = "1.0.1".to_owned();
    let release_tag = "v1.0.1".to_owned();
    let is_prerelease = false;
    let announcement_body = "# v1.0.1 (2023-01-01)\n\nWow Cool Changelog".to_owned();
    let files = std::collections::HashMap::from([
        ("app1", vec!["src/lib.rs", "dist-manifest.json"]),
        ("app2", vec!["src/client.rs", "dist-manifest.json"]),
    ]);


    // Step 1: Create the client
    let abyss = Gazenot::into_the_abyss(source_host, owner)?;


    // Step 2: Create the Artifact Sets
    let packages = apps.into_iter().map(|app| app.to_owned());
    let artifact_sets = abyss.create_artifact_sets(packages.clone()).await?;


    // Step 3: Upload files
    let uploads = artifact_sets.iter().filter_map(|set| {
        // Gather up all the files we want to upload to each ArtifactSet
        files.get(set.package.as_str()).map(|files| {
            let files = files.iter().map(Utf8PathBuf::from).collect();
            (set, files)
        })
    });
    abyss.upload_files(uploads).await?;


    // Step 4: Create Releases
    let releases = artifact_sets.iter().map(|set| {
        let release = ReleaseKey {
            version: release_version.clone(),
            tag: release_tag.clone(),
            is_prerelease,
        };
        (set, release)
    });
    let _releases = abyss.create_releases(releases).await?;


    // Step 5: Announce Releases
    //
    // You could use the `releases` from the previous step, or upgrade the announcement_sets to
    // releases as we do here, for situations where these steps happen on different machines.
    let releases = artifact_sets
        .iter()
        .map(|set| set.to_release(release_tag.clone()))
        .collect::<Vec<_>>();
    let announcement = AnnouncementKey {
        body: announcement_body,
    };
    abyss.create_announcements(&releases, announcement).await?;

    Ok(())
}
```

Note that in typical usage, steps 2, 3, 4, and 5 would all
be done on different machines (as they are different stages
of cargo-dist's release pipeline in CI). In that case, you
would have a workflow like:

* Plan machine: step 1 + step 2, then serialize and store the ArtifactSets
* Hosting machine: load and deserialize ArtifactSets, then step 1 + step 3
* Publish machine: load and deserialize ArtifactSets, then step 1 + step 4
* Announce machine: load and deserialize ArtifactSets, then step 1 + step 5
