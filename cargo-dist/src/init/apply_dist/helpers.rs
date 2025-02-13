use crate::platform::MinGlibcVersion;
use axoasset::toml_edit;
use tracing::trace;

pub fn skip_optional_value<I>(
    _table: &mut toml_edit::Table,
    key: &str,
    _desc: &str,
    _val: Option<I>,
) {
    trace!("apply_dist/skipping: {}", key);
}

pub fn skip_string_list<I>(_table: &mut toml_edit::Table, key: &str, _desc: &str, _list: Option<I>) {
    trace!("apply_dist/skipping: {}", key);
}

/// Update the toml table to add/remove this value
///
/// If the value is Some we will set the value and hang a description comment off of it.
/// If the given key already existed in the table, this will update it in place and overwrite
/// whatever comment was above it. If the given key is new, it will appear at the end of the
/// table.
///
/// If the value is None, we delete it (and any comment above it).
pub fn apply_optional_value<I>(table: &mut toml_edit::Table, key: &str, desc: &str, val: Option<I>)
where
    I: Into<toml_edit::Value>,
{
    if let Some(val) = val {
        table.insert(key, toml_edit::value(val));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_optional_value`][] but with a list of items to `.to_string()`
pub fn apply_string_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        let array: toml_edit::Array = items.into_iter().collect();
        // FIXME: Break the array up into multiple lines with pretty formatting
        // if the list is "too long". Alternatively, more precisely toml-edit
        // the existing value so that we can preserve the user's formatting and comments.
        table.insert(key, toml_edit::Item::Value(toml_edit::Value::Array(array)));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_string_list`][] but when the list can be shorthanded as a string
pub fn apply_string_or_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        if items.len() == 1 {
            apply_optional_value(table, key, desc, items.into_iter().next())
        } else {
            apply_string_list(table, key, desc, Some(items))
        }
    } else {
        table.remove(key);
    }
}

/// Similar to [`apply_optional_value`][] but specialized to `MinGlibcVersion`, since we're not able to work with structs dynamically
pub fn apply_optional_min_glibc_version(
    table: &mut toml_edit::Table,
    key: &str,
    desc: &str,
    val: Option<&MinGlibcVersion>,
) {
    if let Some(min_glibc_version) = val {
        let new_item = &mut table[key];
        let mut new_table = toml_edit::table();
        if let Some(new_table) = new_table.as_table_mut() {
            for (target, version) in min_glibc_version {
                new_table.insert(target, toml_edit::Item::Value(version.to_string().into()));
            }
            new_table.decor_mut().set_prefix(desc);
        }
        new_item.or_insert(new_table);
    } else {
        table.remove(key);
    }
}
