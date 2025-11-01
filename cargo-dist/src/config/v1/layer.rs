//! Utils for merging things

use serde::{Deserialize, Serialize};

/// Trait for merging a new layer of config
pub trait ApplyLayer
where
    Self: Sized,
{
    /// The much more Option-ridden version of this config
    /// that can be repeatedly layerd with options
    type Layer;

    /// Merges this value with another layer of itself, preferring the new layer
    fn apply_layer(&mut self, layer: Self::Layer);

    /// Merges this value with another layer of itself, preferring the new layer
    ///
    /// (asymmetric case where the rhs is an Option but we're just A Value)
    fn apply_val_layer(&mut self, layer: Option<Self::Layer>) {
        if let Some(val) = layer {
            self.apply_layer(val);
        }
    }
}

/// Extension trait to provide apply_bool_layer
pub trait ApplySelfLayerExt {
    /// inner type
    type Inner;
    /// Merge an `Option<Layer>` with an `Option<BoolOr<Layer>>`
    ///
    /// There are 3 cases for the rhs (layer):
    ///
    /// * Some(Val): override; recursively apply_layer
    /// * Some(false): manually disabled; set lhs to None
    /// * Some(true) / None: redundant; do nothing
    ///
    /// There are 2 cases for the lhs (self):
    ///
    /// * Some: still live, can be overriden/merged
    /// * None: permanently disabled, rhs will be ignored
    fn apply_opt_layer(&mut self, layer: Option<Self::Inner>);
}

impl<T> ApplySelfLayerExt for Option<T>
where
    T: ApplyLayer<Layer = T>,
{
    type Inner = T;
    /// Merges this value with another layer of itself, preferring the new layer
    ///
    /// (asymteric case where the rhs is an Option but we're just A Value)
    fn apply_opt_layer(&mut self, layer: Option<Self::Inner>) {
        if let Some(val) = layer {
            if let Some(this) = self {
                this.apply_layer(val);
            } else {
                *self = Some(val);
            }
        }
    }
}

/// Extension trait to provide apply_bool_layer
pub trait ApplyBoolLayerExt {
    /// inner type
    type Inner;
    /// Merge an `Option<Layer>` with an `Option<BoolOr<Layer>>`
    ///
    /// There are 3 cases for the rhs (layer):
    ///
    /// * Some(Val): override; recursively apply_layer
    /// * Some(false): manually disabled; set lhs to None
    /// * Some(true) / None: redundant; do nothing
    ///
    /// There are 2 cases for the lhs (self):
    ///
    /// * Some: still live, can be overriden/merged
    /// * None: permanently disabled, rhs will be ignored
    fn apply_bool_layer(&mut self, layer: Option<BoolOr<Self::Inner>>);
}

impl<T> ApplyBoolLayerExt for Option<T>
where
    T: ApplyLayer + Default,
{
    type Inner = T::Layer;

    /// Apply a layer that can either be a boolean, or a Layer Value (most likely an object).
    ///
    /// Possible cases (lhs is the resultant config, rhs is the incoming layer):
    /// lhs == Some && rhs == true  = nothing happens
    /// lhs == Some && rhs == false = lhs gets set to None
    /// lhs == Some && rhs == value = layer gets applied to lhs
    /// lhs == None && rhs == true  = lhs gets set to layer default
    /// lhs == None && rhs == false = nothing happens
    /// lhs == None && rhs == value = lhs gets set to layer default with layer applied
    /// rhs = nothing               = we do nothing
    fn apply_bool_layer(&mut self, layer: Option<BoolOr<Self::Inner>>) {
        match layer {
            Some(BoolOr::Val(val)) => {
                if let Some(this) = self {
                    this.apply_layer(val);
                } else {
                    let mut t = T::default();
                    t.apply_layer(val);
                    *self = Some(t);
                }
            }
            Some(BoolOr::Bool(false)) => {
                // Disable this setting
                *self = None;
            }
            Some(BoolOr::Bool(true)) => {
                // Enable if self was previously set to None
                if self.is_none() {
                    *self = Some(T::default());
                }
            }
            None => {}
        }
    }
}

/// Extension trait to provide apply_val
pub trait ApplyValExt
where
    Self: Sized,
{
    /// Merges a `T` with an `Option<T>`
    ///
    /// Overwrites the lhs if the rhs is Some
    fn apply_val(&mut self, layer: Option<Self>);
}
impl<T> ApplyValExt for T {
    fn apply_val(&mut self, layer: Option<Self>) {
        if let Some(val) = layer {
            *self = val;
        }
    }
}

/// Extension trait to provide apply_opt
pub trait ApplyOptExt
where
    Self: Sized,
{
    /// Merges an `Option<T>` with an `Option<T>`
    ///
    /// Overwrites the lhs if the rhs is Some
    fn apply_opt(&mut self, layer: Self);
}
impl<T> ApplyOptExt for Option<T> {
    fn apply_opt(&mut self, layer: Self) {
        if let Some(val) = layer {
            *self = Some(val);
        }
    }
}

/// A value or just a boolean
///
/// This allows us to have a simple yes/no version of a config while still
/// allowing for a more advanced version to exist.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum BoolOr<T> {
    /// They gave the simple bool
    Bool(bool),
    /// They gave a more interesting value
    Val(T),
}
