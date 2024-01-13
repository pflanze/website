//! These are parts from the html / html-sys crates, <https://github.com/yoshuawuyts/html>

//! Copyright 2020 Yoshua Wuyts, license MIT or Apache as per the project above.

use kstring::KString;
use serde::{Deserialize, Serialize};

/// An attribute
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: KString,
    pub description: KString,
    pub field_name: KString,
    pub ty: AttributeType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttributeType {
    Bool,
    KString,
    Integer,
    Float,
    Identifier(KString),
    Enumerable(Vec<KString>),
}


/// The final source of truth we used to generate code from.
///
/// Created by combining all of the previously parsed data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedElement {
    pub tag_name: KString,
    pub struct_name: KString,
    pub submodule_name: KString,
    pub mdn_link: KString,
    pub has_global_attributes: bool,
    pub has_closing_tag: bool,
    pub attributes: Vec<Attribute>,
    pub dom_interface: KString,
    pub content_categories: Vec<MergedCategory>,
    pub permitted_child_elements: Vec<KString>,
}

/// Each element in HTML falls into zero or more categories that group elements
/// with similar characteristics together.
///
/// Unlike `ParsedCategory`, this can no longer hold any child elements.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MergedCategory {
    Metadata,
    Flow,
    Sectioning,
    Heading,
    Phrasing,
    Embedded,
    Interactive,
    Palpable,
    ScriptSupporting,
    Transparent,
}

