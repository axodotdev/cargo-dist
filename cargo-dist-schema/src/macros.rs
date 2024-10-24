//! Internal macros for cargo-dist

/// ## Motivation
///
/// cargo-dist deals with a lot of "string-like" types. A target triple,
/// like `x86_64-unknown-linux-gnu`, for example, is string-like. So is
/// a github runner name, like `macos-14`.
///
/// Declaring `target` fields to be of type `String` might sound fine,
/// but when you're looking at:
///
/// ```rust,ignore
///   let mystery_var: BTreeMap<String, BTreeMap<String, Vec<String>>>;
/// ```
///
/// how do you know what each of those `String` refer to?
///
/// Rust lets you declare type aliases, so you might do:
///
/// ```rust,ignore
///  type TargetTriple = String;
/// ```
///
/// And then the type of our mystery_var becomes a little clearer:
///
/// ```rust,ignore
///   let mystery_var: BTreeMap<TargetTriple, BTReeMap<String, Vec<String>>>;
/// ```
///
/// However, this is a small cosmetic difference: we didn't gain any actual
/// type safety.
///
/// We can still very much assign it things that are completely unrelated:
///
/// ```rust,ignore
/// type TargetTriple = String;
/// type GitHubRunner = String;
///
/// let mut target: TargetTriple = "x86_64-unknown-linux-gnu".to_owned(); // so far so good
/// let runner: GithubRunner = "macos-14".to_owned(); // that's okay too
/// target = runner; // 💥 uh oh! this compiles, but it shouldn't!
/// target = "complete nonsense".into(); // 😭 no, code, what are you doing! compiler help us!
/// ```
///
/// If we want those two types to be actually distinct, we have to make "new types" for them.
/// We could make a struct with a field:
///
/// ```rust,ignore
/// pub struct TargetTriple {
///    pub value: String,
/// }
///
/// let t: TargetTriple = get_target();
/// eprintln!("our target is {}", t.value);
/// ```
///
/// But that's a bit wordy — we're always ever going to have one field. The Rust pattern
/// most commonly used here is to use a "tuple struct": think of it as a struct with numbered
/// fields: in this case, it has a single field, named `0`
///
/// ```rust,ignore
/// pub struct TargetTriple(String);
///
/// let t: TargetTriple = get_target();
/// eprintln!("our target is {}", t.0);
/// ```
///
/// With this technique, it's impossible to _accidentally_ assign a `GithubRunner`
/// to a `TargetTriple`, for example:
///
/// ```rust,ignore
/// pub struct TargetTriple(String);
/// pub struct GithubRunner(String);
///
/// let mut target: TargetTriple = get_target();
/// let runner: GithubRunner = get_runner();
/// target = runner; // ✨ this is now a compile error!
/// ```
///
/// We now have the compiler's back. We can now use rust-analyzer's "Find all references"
/// functionality on `TargetTriple` and find all the places in the codebase
/// we care about targets!
///
/// But we usually want to do _more_ with these types. Just like we're
/// able to compare `String`s for equality, and order them, and hash
/// them, and clone them, we also want to be able to do that for types
/// like `TargetTriple` and `GithubRunner`.
///
/// We also want to be able to build references to them, perhaps some from
/// some static string. `String` has the corresponding unsized type `str`,
/// and `String::as_ref()` returns a `&str` — we need some sort of similar
/// mapping here.
///
/// Doing all this by hand, exactly correct, every time, for every one of
/// those types, is tricky. `String` and `&str` are linked together with
/// multiple `From`, `AsRef`, and `Deref` implementations: it's really easy
/// to forget one.
///
/// So, this macro does all that for you!
///
/// ## Usage
///
/// Let's review what you need to know to use a type declared by this macro.
///
/// ### Declaring a new type
///
/// You can invoke this macro to declare one or more "strongly-typed string"
/// types, like so:
///
/// ```rust,ignore
/// declare_strongly_typed_string! {
///   /// TargetTriple docs go here
///   pub const TargetTriple => &TargetTripleRef;
///
///   /// GithubRunner docs go here
///   pub const GithubRunner => &GithubRunner;
/// }
/// ```
///
/// ### Taking values of that type
///
/// Let's assume we're talking about `TargetTriple`: if you'd normally use
/// the `String` type, (ie. you need ownership of that type, maybe you're
/// storing it in a struct), then you'll want `TargetTriple` itself:
///
/// ```rust,ignore
/// struct Blah {
///   targets: Vec<String>;
/// }
/// // 👇 becomes
/// struct Blah {
///   targets: Vec<TargetTriple>;
/// }
/// ```
///
/// If you're only reading from it, then maybe you can take a `&TargetTripleRef` instead:
///
/// ```rust,ignore
/// fn is_target_triple_funny(target: &str) -> bool {
///   target.contains("loong") // let's be honest: it's kinda funny
/// }
/// // 👇 becomes
/// fn is_target_triple_funny(target: &TargetTripleRef) -> bool {
///   target.as_str().contains("loong")
/// }
/// ```
///
/// You don't _have_ to, but it lets you take values built from 'static strings,
/// which... guess what the next section is about?
///
/// ### Creating values of that type
///
/// You can create owned values with `::new()`:
///
/// ```rust,ignore
/// let target = String::from("x86_64-unknown-linux-gnu");
/// // 👇 becomes
/// let target = TargetTriple::new("x86_64-unknown-linux-gnu");
/// ```
///
/// And now you can "Find all reference" for `TargetTriple::new` to find
/// all the places in the codebase where you're turning "user input" into
/// such a value.
///
/// You can still mess up, but it takes effort, and it's easier to find
/// places to review.
///
/// You can also create references with `::from_str()`:
///
/// ```rust,ignore
/// let target = TargetTriple::from_str("x86_64-unknown-linux-gnu");
/// // 👇 becomes
/// let target = TargetTriple::new("x86_64-unknown-linux-gnu");
/// ```
///
/// What you're getting here is a `&'static TargetTripleRef` — no allocations
/// involved, and if your functions take `&TargetTripleRef`, then you're already
/// all set.
///
/// ### Treating it as a string anyway
///
/// You can access the underlying value with `::as_str()`:
///
/// ```rust,ignore
/// let target = String::from("x86_64-unknown-linux-gnu");
/// let first_token = target.split('-').next().unwrap();
/// // 👇 becomes
/// let target = TargetTriple::new("x86_64-unknown-linux-gnu");
/// let first_token = target.as_str().split('-').next().unwrap();
/// ```
///
/// Of course, the `String/&str` version is shorter — the whole thing
/// is to _discourage_ you from treating that value as a string: to have
/// it live as a string as short as possible, to avoid mistakes.
///
/// ### Adding methods
///
/// Because `TargetTriple` is a type we declare ourselves, as opposed to
/// `String`, which is declared in the standard library, we can define
/// our own methods on it, like so:
///
/// ```rust,ignore
/// impl TargetTriple {
///     pub fn tokens(&self) -> impl Iterator<Item = &str> {
///         self.as_str().split('-')
///     }
/// }
/// ```
///
/// And then the transformation above would look more like:
///
/// ```rust,ignore
/// let target = String::from("x86_64-unknown-linux-gnu");
/// let first_token = target.split('-').next().unwrap();
/// // 👇 becomes
/// let target = TargetTriple::new("x86_64-unknown-linux-gnu");
/// let first_token = target.tokens().next().unwrap();
/// ```
///
/// Now we're not duplicating the logic of "splitting on '-'" in a bunch
/// of places. Of course, it's unlikely that target triples will suddenly
/// switched to em-dash as a separator, but you get the gist.
///
/// Note that even the code above `target.tokens()` is a bit too
/// stringly-typed: we could have a `.as_parsed()` method that returns
/// a struct like `ParsedTriple`, which has separate fields for
/// "os", "arch", "abigunk", etc. — again, there would be only one
/// path from `TargetTriple` to `ParsedTriple`, which would be easy to
/// search to, the logic for transforming one into the other would be
/// in a single place, etc. You get it.
///
/// ### Annoying corner cases: slices
///
/// This will not work:
///
/// ```rust,ignore
/// fn i_take_a_slice(targets: &[TargetTripleRef]) { todo!(targets) }
///
/// let targets = vec![TargetTriple::new("x86_64-unknown-linux-gnu")];
/// i_take_a_slice(&targets);
/// ```
///
/// Because you have a `&Vec<TargetTriple>`, and `Deref` only takes you
/// as far as `&[TargetTriple]`, but not `&[TargetTripleRef]`. If you
/// encounter that case, it's probably fine to just take a `&[TargetTriple]`.
///
/// Note that you would have the same problem with `Vec<String>`: it would give
/// you a `&[String]`, not a `&[&str]`. You could take an `impl Iterator<Item = &str>`
/// if you really wanted to.
///
/// ### Annoying corner case: match
///
/// This will not work:
///
/// ```rust,ignore
/// fn match_on_target(target: &TargetTripleRef) => &str {
///   match target {
///     TARGET_X64_WINDOWS => "what's up gamers",
///     _ => "good morning",
///   }
/// }
/// ```
///
/// Even if `TARGET_X64_WINDOWS` is a `&'static TargetTripleRef` and
/// a `const`. Doesn't matter. rustc says no. Maybe in the future.
///
/// For now, just stick it in a `HashMap`, or use an if-else chain or something. Sorry!
#[macro_export]
macro_rules! declare_strongly_typed_string {
    ($(
        $(#[$attr:meta])*
        $vis:vis struct $name:ident => &$ref_name:ident;
    )+) => {
        $(
            #[derive(Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
            #[derive(serde::Serialize, serde::Deserialize)]
            #[derive(schemars::JsonSchema)]
            #[serde(transparent)]
            #[repr(transparent)]
            $(#[$attr])*
            pub struct $name(String);

            #[automatically_derived]
            impl $name {
                /// Constructs a new strongly-typed value
                #[inline]
                pub const fn new(raw: String) -> Self {
                    Self(raw)
                }

                #[doc = "Turn $name into $ref_name explicitly"]
                #[inline]
                pub fn as_explicit_ref(&self) -> &$ref_name {
                    &self
                }
            }

            #[automatically_derived]
            impl ::std::borrow::Borrow<$ref_name> for $name {
                #[inline]
                fn borrow(&self) -> &$ref_name {
                    ::std::ops::Deref::deref(self)
                }
            }

            #[automatically_derived]
            impl ::std::convert::AsRef<$ref_name> for $name {
                #[inline]
                fn as_ref(&self) -> &$ref_name {
                    ::std::ops::Deref::deref(self)
                }
            }

            #[automatically_derived]
            impl ::std::convert::AsRef<str> for $name {
                #[inline]
                fn as_ref(&self) -> &str {
                    self.as_str()
                }
            }

            #[automatically_derived]
            impl ::std::str::FromStr for $name {
                type Err = ::std::convert::Infallible;
                #[inline]
                fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                    ::std::result::Result::Ok($name::new(s.to_owned()))
                }
            }

            #[automatically_derived]
            impl ::std::borrow::Borrow<str> for $name {
                #[inline]
                fn borrow(&self) -> &str {
                    self.as_str()
                }
            }

            #[automatically_derived]
            impl ::std::ops::Deref for $name {
                type Target = $ref_name;
                #[inline]
                fn deref(&self) -> &Self::Target {
                    $ref_name::from_str(::std::convert::AsRef::as_ref(&self.0))
                }
            }

            #[automatically_derived]
            impl ::std::fmt::Debug for $name {
                #[inline]
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    <$ref_name as ::std::fmt::Debug>::fmt(::std::ops::Deref::deref(self), f)
                }
            }

            #[automatically_derived]
            impl ::std::fmt::Display for $name {
                #[inline]
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    <$ref_name as ::std::fmt::Display>::fmt(::std::ops::Deref::deref(self), f)
                }
            }

            #[repr(transparent)]
            #[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
            $(#[$attr])*
            pub struct $ref_name(str);

            #[automatically_derived]
            impl $ref_name {
                #[allow(unsafe_code)]
                #[inline]
                #[doc = "Transparently reinterprets the string slice as a strongly-typed $ref_name"]
                pub const fn from_str(raw: &str) -> &Self {
                    let ptr: *const str = raw;

                    // SAFETY: `$ref_name` is `#[repr(transparent)]` around a single `str` field, so a `*const str` can be safely reinterpreted as a `*const $ref_name`
                    unsafe { &*(ptr as *const Self) }
                }

                #[doc = r" Provides access to the underlying value as a string slice."]
                #[inline]
                pub const fn as_str(&self) -> &str {
                    &self.0
                }
            }

            #[automatically_derived]
            impl ::std::borrow::ToOwned for $ref_name {
                type Owned = $name;
                #[inline]
                fn to_owned(&self) -> Self::Owned {
                    $name(self.0.into())
                }
            }

            #[automatically_derived]
            impl ::std::cmp::PartialEq<$ref_name> for $name {
                #[inline]
                fn eq(&self, other: &$ref_name) -> bool {
                    self.as_str() == other.as_str()
                }
            }

            #[automatically_derived]
            impl ::std::cmp::PartialEq<$name> for $ref_name {
                #[inline]
                fn eq(&self, other: &$name) -> bool {
                    self.as_str() == other.as_str()
                }
            }

            #[automatically_derived]
            impl ::std::cmp::PartialEq<&'_ $ref_name> for $name {
                #[inline]
                fn eq(&self, other: &&$ref_name) -> bool {
                    self.as_str() == other.as_str()
                }
            }

            #[automatically_derived]
            impl ::std::cmp::PartialEq<$name> for &'_ $ref_name {
                #[inline]
                fn eq(&self, other: &$name) -> bool {
                    self.as_str() == other.as_str()
                }
            }

            #[automatically_derived]
            impl<'a> ::std::convert::From<&'a str> for &'a $ref_name {
                #[inline]
                fn from(s: &'a str) -> &'a $ref_name {
                    $ref_name::from_str(s)
                }
            }

            #[automatically_derived]
            impl ::std::borrow::Borrow<str> for $ref_name {
                #[inline]
                fn borrow(&self) -> &str {
                    &self.0
                }
            }

            #[automatically_derived]
            impl ::std::convert::AsRef<str> for $ref_name {
                #[inline]
                fn as_ref(&self) -> &str {
                    &self.0
                }
            }

            #[automatically_derived]
            impl ::std::convert::From<&'_ $ref_name> for ::std::rc::Rc<$ref_name> {
                #[allow(unsafe_code)]
                #[inline]
                fn from(r: &'_ $ref_name) -> Self {
                    // SAFETY: `$ref_name` is `#[repr(transparent)]` around a single `str` field, so a `*const str` can be safely reinterpreted as a `*const $ref_name`
                    let rc = ::std::rc::Rc::<str>::from(r.as_str());
                    unsafe { ::std::rc::Rc::from_raw(::std::rc::Rc::into_raw(rc) as *const $ref_name) }
                }
            }
            #[automatically_derived]
            impl ::std::convert::From<&'_ $ref_name> for ::std::sync::Arc<$ref_name> {
                #[allow(unsafe_code)]
                #[inline]
                fn from(r: &'_ $ref_name) -> Self {
                    // SAFETY: `$ref_name` is `#[repr(transparent)]` around a single `str` field, so a `*const str` can be safely reinterpreted as a `*const $ref_name`
                    let arc = ::std::sync::Arc::<str>::from(r.as_str());
                    unsafe {
                        ::std::sync::Arc::from_raw(::std::sync::Arc::into_raw(arc) as *const $ref_name)
                    }
                }
            }

            #[automatically_derived]
            impl ::std::fmt::Debug for $ref_name {
                #[inline]
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    <str as ::std::fmt::Debug>::fmt(&self.0, f)
                }
            }

            #[automatically_derived]
            impl ::std::fmt::Display for $ref_name {
                #[inline]
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    <str as ::std::fmt::Display>::fmt(&self.0, f)
                }
            }
        )+
    };
}

/// Some values look a lot like strings, like the "windows" in "i686-pc-windows-msvc"
/// or the "linux" in "x86_64-unknown-linux-gnu" — but in practice, we have special
/// handling for just a handful of possible values.
///
/// In that case, instead of having a `String` field, it's useful to have an enum with
/// all the variants we know/care about, and a "fallback" variant for the ones we don't
/// know about.
#[macro_export]
macro_rules! declare_stringish_enum {
    (
        $(
            $(#[$enum_meta:meta])*
            $vis:vis enum $name:ident {
                $(#[$other_meta:meta])*
                Other(String),
                $($(#[$meta:meta])* $variant:ident = $str:expr,)+
            }
        )+
    ) => {
        $(
            #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
            $(#[$enum_meta])*
            $vis enum $name {
                $($(#[$meta])* $variant,)*
                $(#[$other_meta])*
                Other(::std::string::String),
            }

            impl ::std::fmt::Display for $name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    match self {
                        $(Self::$variant => write!(f, "{}", $str),)*
                        Self::Other(s) => write!(f, "{}", s),
                    }
                }
            }

            impl $name {
                fn from_str(s: &str) -> Self {
                    match s {
                        $($str => Self::$variant,)*
                        other => Self::Other(other.to_string()),
                    }
                }
            }
        )+
    };
}
