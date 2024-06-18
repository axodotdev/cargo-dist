use std::fmt;

use crate::errors::*;

use url::Url;

#[derive(Debug)]
pub enum GithubRepoInput {
    Url(String),
    Ssh(String),
}

/// Represents a GitHub repository that we can query things about.
#[derive(Debug, Clone)]
pub struct GithubRepo {
    /// The repository owner.
    pub owner: String,
    /// The repository name.
    pub name: String,
}

impl GithubRepo {
    /// Returns a URL suitable for web access to the repository.
    pub fn web_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.name)
    }

    /// Constructs a new Github repository from a "owner/name" string. Notably, this does not check
    /// whether the repo actually exists.
    pub fn from_url(repo_url: &str) -> Result<Self> {
        GithubRepoInput::new(repo_url.to_string())?.parse()
    }
}

impl fmt::Display for GithubRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}/{})", self.owner, self.name)
    }
}

impl GithubRepoInput {
    pub fn new(repo_string: String) -> Result<Self> {
        // Handle git+https just the same as https
        if repo_string.starts_with("https") || repo_string.starts_with("git+https") {
            Ok(Self::Url(repo_string))
        } else if repo_string.starts_with("git@") {
            Ok(Self::Ssh(repo_string))
        } else {
            let err = AxoprojectError::UnknownRepoStyle { url: repo_string };
            Err(err)
        }
    }

    pub fn parse(self) -> Result<GithubRepo> {
        match self {
            Self::Url(s) => Ok(Self::parse_url(s)?),
            Self::Ssh(s) => Ok(Self::parse_ssh(s)?),
        }
    }

    fn parse_url(repo_string: String) -> Result<GithubRepo> {
        let parsed = Url::parse(&repo_string)?;
        if parsed.domain() != Some("github.com") {
            return Err(AxoprojectError::NotGitHubError { url: repo_string });
        }
        let segment_list = parsed.path_segments().map(|c| c.collect::<Vec<_>>());
        if let Some(segments) = segment_list {
            if segments.len() >= 2 {
                let owner = segments[0].to_string();
                let name = Self::remove_git_suffix(segments[1].to_string());
                let rest_is_empty = segments.iter().skip(2).all(|s| s.trim().is_empty());
                if rest_is_empty {
                    return Ok(GithubRepo { owner, name });
                }
            }
        }
        Err(AxoprojectError::RepoParseError { repo: repo_string })
    }

    fn parse_ssh(repo_string: String) -> Result<GithubRepo> {
        let core = Self::remove_git_suffix(Self::remove_git_prefix(repo_string.clone())?);
        let segments: Vec<&str> = core.split('/').collect();
        if !segments.is_empty() && segments.len() >= 2 {
            let owner = segments[0].to_string();
            let name = Self::remove_git_suffix(segments[1].to_string());
            let rest_is_empty = segments.iter().skip(2).all(|s| s.trim().is_empty());
            if rest_is_empty {
                return Ok(GithubRepo { owner, name });
            }
        }
        Err(AxoprojectError::RepoParseError { repo: repo_string })
    }

    fn remove_git_prefix(s: String) -> Result<String> {
        let prefix = "git@github.com:";
        if let Some(stripped) = s.strip_prefix(prefix) {
            Ok(stripped.to_string())
        } else {
            Err(AxoprojectError::NotGitHubError { url: s })
        }
    }

    fn remove_git_suffix(s: String) -> String {
        if let Some(chomped) = s.strip_suffix(".git") {
            chomped.to_string()
        } else {
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_an_https_repo_string() {
        let input = "https://github.com/axodotdev/oranda";
        let actual_owner = "axodotdev";
        let actual_name = "oranda";
        let parsed = GithubRepo::from_url(input).unwrap();
        assert_eq!(parsed.owner, actual_owner);
        assert_eq!(parsed.name, actual_name);
    }

    #[test]
    fn it_parses_an_https_repo_string_with_dot_git() {
        let input = "https://github.com/axodotdev/oranda.git";
        let actual_owner = "axodotdev";
        let actual_name = "oranda";
        let parsed = GithubRepo::from_url(input).unwrap();
        assert_eq!(parsed.owner, actual_owner);
        assert_eq!(parsed.name, actual_name);
    }

    #[test]
    fn it_parses_an_ssh_repo_string() {
        let input = "git@github.com:axodotdev/oranda.git";
        let actual_owner = "axodotdev";
        let actual_name = "oranda";
        let parsed = GithubRepo::from_url(input).unwrap();
        assert_eq!(parsed.owner, actual_owner);
        assert_eq!(parsed.name, actual_name);
    }
}
