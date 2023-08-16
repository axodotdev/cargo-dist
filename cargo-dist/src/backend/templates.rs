//! Logic for resolving/rendering templates

use camino::{Utf8Path, Utf8PathBuf};
use include_dir::{include_dir, Dir};
use minijinja::Environment;
use newline_converter::dos2unix;
use serde::Serialize;

use crate::{errors::DistResult, SortedMap};

const TEMPLATE_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");
/// Key used for looking up templates (relative path from the templates dir)
pub type TemplateId = &'static str;
/// Template key for installer.ps1
pub const TEMPLATE_INSTALLER_PS1: TemplateId = "installer/installer.ps1";
/// Template key for installer.sh
pub const TEMPLATE_INSTALLER_SH: TemplateId = "installer/installer.sh";
/// Template key for Homebrew formula
pub const TEMPLATE_INSTALLER_RB: TemplateId = "installer/homebrew.rb";
/// Template key for the npm installer dir
pub const TEMPLATE_INSTALLER_NPM: TemplateId = "installer/npm";
/// Template key for the github ci.yml
pub const TEMPLATE_CI_GITHUB: TemplateId = "ci/github_ci.yml";

/// ID used to look up an environment in [`Templates::envs`][]
type EnvId = &'static str;
/// Vanilla environment for most things
const ENV_MISC: &str = "*";
/// Environment with tweaked syntax to deal with {{ blah }} showing up in templated yml files
const ENV_YAML: &str = "yml";

/// Main templates struct that gets passed around in the application.
#[derive(Debug)]
pub struct Templates {
    /// Minijinja environments that contains all loaded templates
    ///
    /// Keys are ENV_MISC, ENV_YML
    envs: SortedMap<EnvId, Environment<'static>>,
    /// Traversable/searchable structure of the templates dir
    entries: TemplateDir,
}

/// An entry in the template dir
#[derive(Debug)]
pub enum TemplateEntry {
    /// A directory
    Dir(TemplateDir),
    /// A file
    File(TemplateFile),
}

/// A directory in the template dir
#[derive(Debug)]
pub struct TemplateDir {
    /// name of the dir
    _name: String,
    /// relative path of the dir from `TEMPLATE_DIR`
    ///
    /// (This is also the [`TemplateId`][] for this dir)
    pub path: Utf8PathBuf,
    /// children
    pub entries: SortedMap<String, TemplateEntry>,
}

/// A file in the template dir
#[derive(Debug)]
pub struct TemplateFile {
    /// name of the file
    pub name: String,
    /// relative path of the file from `TEMPLATE_DIR`
    ///
    /// (This is also the [`TemplateId`][] for this file)
    pub path: Utf8PathBuf,
    /// which Environment will render this
    env: EnvId,
}

impl TemplateFile {
    /// Gets the relative path to this file from the ancestor directory
    pub fn path_from_ancestor(&self, ancestor: &TemplateDir) -> &Utf8Path {
        self.path
            .strip_prefix(&ancestor.path)
            .expect("jinja2 template path wasn't properly nested under parent")
    }
}

impl Templates {
    /// Load + Parse templates from the binary
    pub fn new() -> DistResult<Self> {
        // Initialize the envs
        let mut envs = SortedMap::new();
        {
            let misc_env = Environment::new();
            envs.insert(ENV_MISC, misc_env);
        }
        {
            // Github CI ymls already use {{ }} as delimiters so add an extra layer
            // of braces to disambiguate without needing tons of escaping
            let mut yaml_env = Environment::new();
            yaml_env
                .set_syntax(minijinja::Syntax {
                    block_start: "{{%".into(),
                    block_end: "%}}".into(),
                    variable_start: "{{{".into(),
                    variable_end: "}}}".into(),
                    comment_start: "{{#".into(),
                    comment_end: "#}}".into(),
                })
                .expect("failed to change jinja2 syntax for yaml files");
            envs.insert(ENV_YAML, yaml_env);
        }
        for env in envs.values_mut() {
            env.set_debug(true);

            fn jinja_error(details: String) -> std::result::Result<String, minijinja::Error> {
                Err(minijinja::Error::new(
                    minijinja::ErrorKind::EvalBlock,
                    details,
                ))
            }

            env.add_function("error", jinja_error);
        }

        let mut entries = TemplateDir {
            _name: String::new(),
            path: Utf8PathBuf::new(),
            entries: SortedMap::new(),
        };
        // These two `expects` should never happen in production, because all of these things are
        // are baked into the binary. If this fails at all it should presumably *always* fail, and
        // so these unwraps will only show up when someone's messing with the templates locally
        // during development and presumably wrote some malformed jinja2 markup.
        Self::load_files(&mut envs, &TEMPLATE_DIR, &mut entries)
            .expect("failed to load jinja2 templates from binary");

        let templates = Self { envs, entries };

        Ok(templates)
    }

    /// Get the entry for a template by key (the TEMPLATE_* consts)
    fn get_template_entry(&self, key: TemplateId) -> DistResult<&TemplateEntry> {
        let mut parent = &self.entries;
        let mut result: Option<&TemplateEntry> = None;
        for part in key.split('/') {
            result = parent.entries.get(part);
            if let Some(entry) = result {
                if let TemplateEntry::Dir(dir) = entry {
                    parent = dir;
                }
            } else {
                panic!("invalid jinja2 template key: {key}")
            }
        }

        if let Some(entry) = result {
            Ok(entry)
        } else {
            panic!("invalid jinja2 template key: {key}");
        }
    }

    /// Get the entry for a template by key (the TEMPLATE_* consts), and require it to be a file
    pub fn get_template_file(&self, key: TemplateId) -> DistResult<&TemplateFile> {
        if let TemplateEntry::File(file) = self.get_template_entry(key)? {
            Ok(file)
        } else {
            panic!("jinja2 template key was not a file: {key}");
        }
    }

    /// Get the entry for a template by key (the TEMPLATE_* consts), and require it to be a dir
    pub fn get_template_dir(&self, key: TemplateId) -> DistResult<&TemplateDir> {
        if let TemplateEntry::Dir(dir) = self.get_template_entry(key)? {
            Ok(dir)
        } else {
            panic!("jinja2 template key was not a dir: {key}");
        }
    }

    /// Render a template file to a string, cleaning all newlines to be unix-y
    pub fn render_file_to_clean_string(
        &self,
        key: TemplateId,
        val: &impl Serialize,
    ) -> DistResult<String> {
        let file = self.get_template_file(key)?;
        self.render_file_to_clean_string_inner(file, val)
    }

    fn render_file_to_clean_string_inner(
        &self,
        file: &TemplateFile,
        val: &impl Serialize,
    ) -> DistResult<String> {
        let template = self.envs[file.env].get_template(file.path.as_str())?;
        let rendered = template.render(val)?;
        let cleaned = dos2unix(&rendered).into_owned();
        Ok(cleaned)
    }

    /// Render all the templates under a directory to a string, cleaning all newlines to be unix-y
    ///
    /// The output is a map from relpath => rendered_text, where relpath is the path of the file relative
    /// to the starting directory. So if you render "installer", you'll get back "npm/package.json" => "...".
    /// This allows us to store directory structures in the templates dir and forward them verbatim
    /// when writing them to disk.
    pub fn render_dir_to_clean_strings(
        &self,
        key: TemplateId,
        val: &impl Serialize,
    ) -> DistResult<SortedMap<Utf8PathBuf, String>> {
        let root_dir = self.get_template_dir(key)?;
        let mut output = SortedMap::new();
        self.render_dir_to_clean_strings_inner(&mut output, root_dir, root_dir, val)?;
        Ok(output)
    }

    fn render_dir_to_clean_strings_inner(
        &self,
        output: &mut SortedMap<Utf8PathBuf, String>,
        root_dir: &TemplateDir,
        dir: &TemplateDir,
        val: &impl Serialize,
    ) -> DistResult<()> {
        for entry in dir.entries.values() {
            match entry {
                TemplateEntry::Dir(subdir) => {
                    self.render_dir_to_clean_strings_inner(output, root_dir, subdir, val)?
                }
                TemplateEntry::File(file) => {
                    let rendered = self.render_file_to_clean_string_inner(file, val)?;
                    let relpath = file.path_from_ancestor(root_dir);
                    output.insert(relpath.to_owned(), rendered);
                }
            }
        }
        Ok(())
    }

    /// load + parse templates from the binary (recursive)
    fn load_files(
        envs: &mut SortedMap<EnvId, Environment<'static>>,
        dir: &'static Dir,
        parent: &mut TemplateDir,
    ) -> DistResult<()> {
        for entry in dir.entries() {
            let path = Utf8Path::from_path(entry.path()).expect("non-utf8 jinja2 template path");
            if let Some(file) = entry.as_file() {
                if path.extension().unwrap_or_default() != "j2" {
                    // Skip non-jinja-templates (useful for prototyping)
                    continue;
                }
                // Remove the .j2 extension
                let path = path.with_extension("");
                let name = path
                    .file_name()
                    .expect("jinja2 template didn't have a name!?")
                    .to_owned();
                let contents = file
                    .contents_utf8()
                    .expect("non-utf8 jinja2 template")
                    .to_string();
                let env = if path.extension().unwrap_or_default() == "yml" {
                    ENV_YAML
                } else {
                    ENV_MISC
                };

                envs.get_mut(env)
                    .expect("invalid jinja2 env key")
                    .add_template_owned(path.to_string(), contents)
                    .expect("failed to add jinja2 template");
                parent.entries.insert(
                    name.clone(),
                    TemplateEntry::File(TemplateFile { name, path, env }),
                );
            }
            if let Some(dir) = entry.as_dir() {
                let name = path
                    .file_name()
                    .expect("jinja2 template didn't have a name!?")
                    .to_owned();
                let mut new_dir = TemplateDir {
                    _name: name.clone(),
                    path: path.to_owned(),
                    entries: SortedMap::new(),
                };
                Self::load_files(envs, dir, &mut new_dir)
                    .expect("failed to load jinja2 templates from binary");
                parent.entries.insert(name, TemplateEntry::Dir(new_dir));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ensure_known_templates() {
        let templates = Templates::new().unwrap();

        templates.get_template_file(TEMPLATE_INSTALLER_SH).unwrap();
        templates.get_template_file(TEMPLATE_INSTALLER_RB).unwrap();
        templates.get_template_file(TEMPLATE_INSTALLER_PS1).unwrap();
        templates.get_template_dir(TEMPLATE_INSTALLER_NPM).unwrap();

        templates.get_template_file(TEMPLATE_CI_GITHUB).unwrap();
    }
}
