//! Tests for tag parsing
//!
use crate::{parse_tag, Package, ReleaseType, Version};

#[test]
fn parse_one() {
    // "1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_one_v() {
    // "v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_one_package_v() {
    // "axolotlsay-v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay-v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_package() {
    // "axolotlsay-1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay-{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_v_alpha() {
    // "v1.0.0-prerelease.1" in a one package workspace
    let version = "1.0.0-prerelease.1".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_one_package_v_alpha() {
    // "axolotlsay-v1.0.0-prerelease.1" in a one package workspace
    let version = "1.0.0-prerelease.1".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay-v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_prefix_slashv() {
    // "release/v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("release/v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_one_prefix_slash() {
    // "release/1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("release/{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_one_prefix_slash_package_v() {
    // "release/axolotlsay-v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("release/axolotlsay-v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_prefix_slash_package_slashv() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("releases/axolotlsay/v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_package_slashv() {
    // "axolotlsay/v1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay/v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_prefix_slash_package_slash() {
    // "releases/axolotlsay/1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("releases/axolotlsay/{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_prefix_many_slash_package_slash() {
    // "blah/blah/releases/axolotlsay/1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("blah/blah/releases/axolotlsay/{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_one_package_slash() {
    // "axolotlsay/1.0.0" in a one package workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay/{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}

#[test]
fn parse_unified_v() {
    // "v1.0.0" in a unified workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![
        Package {
            name: "axolotlsay".to_owned(),
            version: Some(version.clone()),
        },
        Package {
            name: "otherapp".to_owned(),
            version: Some(version.clone()),
        },
        Package {
            name: "whatever".to_owned(),
            version: Some(version.clone()),
        },
    ];
    let tag = format!("v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn parse_disjoint_v() {
    // selecting the bulk packages in a disjoint workspace
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![
        Package {
            name: "axolotlsay".to_owned(),
            version: Some(version.clone()),
        },
        Package {
            name: "otherapp".to_owned(),
            version: Some(version.clone()),
        },
        Package {
            name: "whatever".to_owned(),
            version: "2.0.0".parse().ok(),
        },
    ];
    let tag = format!("v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
fn ok_parse_one_package_slash() {
    // "axolotlsay/1.0.0" in a one package workspace, when that's not the package
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "asdsadas".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay/{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Version(version));
}

#[test]
#[should_panic]
fn fail_parse_one_package() {
    // "axolotlsay-1.0.0" in a one package workspace, when that's not the package
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "asdsadas".to_owned(),
        version: Some(version.clone()),
    }];
    let tag = format!("axolotlsay-v{version}");

    let _announcing = parse_tag(&packages, &tag).unwrap();
}

#[test]
fn fail_parse_one_package_version_backup() {
    // "axolotlsay-1.0.0" in a one package workspace, when version isn't given as input
    let version = "1.0.0".parse::<Version>().unwrap();
    let packages = vec![Package {
        name: "axolotlsay".to_owned(),
        version: None,
    }];
    let tag = format!("axolotlsay-v{version}");

    let announcing = parse_tag(&packages, &tag).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.release, ReleaseType::Package { idx: 0, version });
}
