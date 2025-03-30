//! Provides a newtype for `toml::map::Map<String, Value>)` with methods
//! to merge (incomplete) configuration data from different sources (files).

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use toml::Value;

use crate::error::LibCfgError;

/// This decides until what depth arrays are merged into the default
/// configuration. Tables are always merged. Deeper arrays replace the default
/// configuration. For our configuration this means, that `scheme` is merged and
/// all other arrays are replaced.
pub(crate) const CONFIG_FILE_MERGE_DEPTH: isize = 2;

/// A newtype holding configuration data.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct CfgVal(toml::map::Map<String, Value>);

/// This API deals with configuration values.
///
impl CfgVal {
    /// Constructor returning an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append key, value pairs from other to `self`.
    ///
    /// ```rust
    /// use tpnote_lib::config_value::CfgVal;
    /// use std::str::FromStr;
    ///
    /// let toml1 = "\
    /// [arg_default]
    /// scheme = 'zettel'
    /// ";
    ///
    /// let toml2 = "\
    /// [base_scheme]
    /// name = 'some name'
    /// ";
    ///
    /// let mut cfg1 = CfgVal::from_str(toml1).unwrap();
    /// let cfg2 = CfgVal::from_str(toml2).unwrap();
    ///
    /// let expected = CfgVal::from_str("\
    /// [arg_default]
    /// scheme = 'zettel'
    /// [base_scheme]
    /// name = 'some name'
    /// ").unwrap();
    ///
    /// // Run test
    /// cfg1.extend(cfg2);
    ///
    /// assert_eq!(cfg1, expected);
    ///
    #[inline]
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    #[inline]
    pub fn insert(&mut self, key: String, val: Value) {
        self.0.insert(key, val); //
    }

    #[inline]
    /// Merges configuration values from `other` into `self`
    /// and returns the result. The top level element is a set of key and value
    /// pairs (map). If one of its values is a `Value::Array`, then the
    /// corresponding array from `other` is appended.
    /// Otherwise the corresponding `other` value replaces the `self` value.
    /// Deeper nested `Value::Array`s are never appended but always replaced
    /// (`CONFIG_FILE_MERGE_PEPTH=2`).
    /// Append key, value pairs from other to `self`.
    ///
    /// ```rust
    /// use tpnote_lib::config_value::CfgVal;
    /// use std::str::FromStr;
    ///
    /// let toml1 = "\
    /// version = '1.0.0'
    /// [[scheme]]
    /// name = 'default'
    /// ";
    /// let toml2 = "\
    /// version = '2.0.0'
    /// [[scheme]]
    /// name = 'zettel'
    /// ";
    ///
    /// let mut cfg1 = CfgVal::from_str(toml1).unwrap();
    /// let cfg2 = CfgVal::from_str(toml2).unwrap();
    ///
    /// let expected = CfgVal::from_str("\
    /// version = '2.0.0'
    /// [[scheme]]
    /// name = 'default'
    /// [[scheme]]
    /// name = 'zettel'
    /// ").unwrap();
    ///
    /// // Run test
    /// let res = cfg1.merge(cfg2);
    ///
    /// assert_eq!(res, expected);
    ///
    pub fn merge(self, other: Self) -> Self {
        let left = Value::Table(self.0);
        let right = Value::Table(other.0);
        let res = Self::merge_toml_values(left, right, CONFIG_FILE_MERGE_DEPTH);
        // Invariant: when left and right are `Value::Table`, then `res`
        // must be a `Value::Table` also.
        if let Value::Table(map) = res {
            Self(map)
        } else {
            unreachable!()
        }
    }

    /// Merges configuration values from the right-hand side into the
    /// left-hand side and returns the result. The top level element is usually
    /// a `toml::Value::Table`. The table is a set of key and value pairs.
    /// The values here can be compound data types, i.e. `Value::Table` or
    /// `Value::Array`.
    /// `merge_depth` controls whether a top-level array in the TOML document
    /// is appended to instead of overridden. This is useful for TOML documents
    /// that have a top-level arrays (`merge_depth=2`) like `[[scheme]]` in
    /// `tpnote.toml`. For top level arrays, one usually wants to append the
    /// right-hand array to the left-hand array instead of just replacing the
    /// left-hand array with the right-hand array. If you set `merge_depth=0`,
    /// all arrays whatever level they have, are always overridden by the
    /// right-hand side.
    pub(crate) fn merge_toml_values(
        left: toml::Value,
        right: toml::Value,
        merge_depth: isize,
    ) -> toml::Value {
        use toml::Value;

        fn get_name(v: &Value) -> Option<&str> {
            v.get("name").and_then(Value::as_str)
        }

        match (left, right) {
            (Value::Array(mut left_items), Value::Array(right_items)) => {
                // The top-level arrays should be merged but nested arrays
                // should act as overrides. For the `tpnote.toml` config,
                // this means that you can specify a sub-set of schemes in
                // an overriding `tpnote.toml` but that nested arrays like
                // `scheme.tmpl.fm_var_localization` are replaced instead
                // of merged.
                if merge_depth > 0 {
                    left_items.reserve(right_items.len());
                    for rvalue in right_items {
                        let lvalue = get_name(&rvalue)
                            .and_then(|rname| {
                                left_items.iter().position(|v| get_name(v) == Some(rname))
                            })
                            .map(|lpos| left_items.remove(lpos));
                        let mvalue = match lvalue {
                            Some(lvalue) => {
                                Self::merge_toml_values(lvalue, rvalue, merge_depth - 1)
                            }
                            None => rvalue,
                        };
                        left_items.push(mvalue);
                    }
                    Value::Array(left_items)
                } else {
                    Value::Array(right_items)
                }
            }
            (Value::Table(mut left_map), Value::Table(right_map)) => {
                if merge_depth > -10 {
                    for (rname, rvalue) in right_map {
                        match left_map.remove(&rname) {
                            Some(lvalue) => {
                                let merged_value =
                                    Self::merge_toml_values(lvalue, rvalue, merge_depth - 1);
                                left_map.insert(rname, merged_value);
                            }
                            None => {
                                left_map.insert(rname, rvalue);
                            }
                        }
                    }
                    Value::Table(left_map)
                } else {
                    Value::Table(right_map)
                }
            }
            (_, value) => value,
        }
    }

    /// Convert to `toml::Value`.
    ///
    /// ```rust
    /// use tpnote_lib::config_value::CfgVal;
    /// use std::str::FromStr;
    ///
    /// let toml1 = "\
    /// version = 1
    /// [[scheme]]
    /// name = 'default'
    /// ";
    ///
    /// let cfg1 = CfgVal::from_str(toml1).unwrap();
    ///
    /// let expected: toml::Value = toml::from_str(toml1).unwrap();
    ///
    /// // Run test
    /// let res = cfg1.to_value();
    ///
    /// assert_eq!(res, expected);
    ///
    pub fn to_value(self) -> toml::Value {
        Value::Table(self.0)
    }
}

impl FromStr for CfgVal {
    type Err = LibCfgError;

    /// Constructor taking a text to deserialize.
    /// Throws an error if the deserialized root element is not a
    /// `Value::Table`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = toml::from_str(s)?;
        if let Value::Table(map) = v {
            Ok(Self(map))
        } else {
            Err(LibCfgError::CfgValInputIsNotTable)
        }
    }
}

impl From<CfgVal> for toml::Value {
    fn from(cfg_val: CfgVal) -> Self {
        cfg_val.to_value()
    }
}
