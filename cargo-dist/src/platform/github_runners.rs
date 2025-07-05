//! Statically-known information about various GitHub Actions runner names

use std::collections::HashMap;

use crate::platform::targets as t;
use dist_schema::{target_lexicon::Triple, GithubRunnerRef, TripleNameRef};
use tracing::warn;

lazy_static::lazy_static! {
    static ref KNOWN_GITHUB_RUNNERS: HashMap<&'static GithubRunnerRef, &'static TripleNameRef> = {
        let mut m = HashMap::new();
        // cf. https://github.com/actions/runner-images/blob/main/README.md
        // last updated 2024-10-25

        //-------- linux
        m.insert(GithubRunnerRef::from_str("ubuntu-22.04"), t::TARGET_X64_LINUX_GNU);
        m.insert(GithubRunnerRef::from_str("ubuntu-24.04"), t::TARGET_X64_LINUX_GNU);
        m.insert(GithubRunnerRef::from_str("ubuntu-latest"), t::TARGET_X64_LINUX_GNU);

        //-------- windows
        m.insert(GithubRunnerRef::from_str("windows-2022"), t::TARGET_X64_WINDOWS);
        m.insert(GithubRunnerRef::from_str("windows-2025"), t::TARGET_X64_WINDOWS);
        m.insert(GithubRunnerRef::from_str("windows-latest"), t::TARGET_X64_WINDOWS);

        //-------- macos x64
        m.insert(GithubRunnerRef::from_str("macos-13"), t::TARGET_X64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-13-large"), t::TARGET_X64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-14-large"), t::TARGET_X64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-15-large"), t::TARGET_X64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-latest-large"), t::TARGET_X64_MAC);

        //-------- macos arm64
        m.insert(GithubRunnerRef::from_str("macos-13-xlarge"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-14"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-14-xlarge"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-15"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-15-xlarge"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-latest"), t::TARGET_ARM64_MAC);
        m.insert(GithubRunnerRef::from_str("macos-latest-xlarge"), t::TARGET_ARM64_MAC);

        m
    };
}

/// Get the target triple for a given GitHub Actions runner (if we know about it)
pub fn target_for_github_runner(runner: &GithubRunnerRef) -> Option<&'static TripleNameRef> {
    if let Some(target) = KNOWN_GITHUB_RUNNERS.get(runner).copied() {
        return Some(target);
    }

    let runner_str = runner.as_str();
    if let Some(rest) = runner_str.strip_prefix("buildjet-") {
        if rest.contains("ubuntu") {
            if rest.ends_with("-arm") {
                return Some(t::TARGET_ARM64_LINUX_GNU);
            } else {
                return Some(t::TARGET_X64_LINUX_GNU);
            }
        }
    }

    None
}

/// Get the target triple for a given GitHub Actions runner (if we know about it), or assume x64-linux-gnu
pub fn target_for_github_runner_or_default(runner: &GithubRunnerRef) -> &'static TripleNameRef {
    const DEFAULT_ASSUMED_TARGET: &TripleNameRef = t::TARGET_X64_LINUX_GNU;

    target_for_github_runner(runner).unwrap_or_else(|| {
        warn!(
            "don't know the triple for github runner '{runner}', assuming {DEFAULT_ASSUMED_TARGET}"
        );
        DEFAULT_ASSUMED_TARGET
    })
}

/// Gets the parsed [`Triple`] for a given GitHub runner.
pub fn triple_for_github_runner_or_default(runner: &GithubRunnerRef) -> Triple {
    // unwrap safety: all the triples in `KNOWN_GITHUB_RUNNERS` should parse
    // cleanly.
    target_for_github_runner_or_default(runner).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_for_github_runner() {
        assert_eq!(
            target_for_github_runner(GithubRunnerRef::from_str("ubuntu-22.04")),
            Some(t::TARGET_X64_LINUX_GNU)
        );
        assert_eq!(
            target_for_github_runner(GithubRunnerRef::from_str("buildjet-8vcpu-ubuntu-2204-arm")),
            Some(t::TARGET_ARM64_LINUX_GNU)
        );
    }
}
