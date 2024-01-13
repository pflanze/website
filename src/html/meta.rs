//! Cleaned up and indexed data for fast DOM correctness verification.

use std::{collections::{HashMap, HashSet}, path::Path, fs::read_dir};
use anyhow::{anyhow, Result, Context};
use kstring::KString;
use crate::html::types::{AttributeType, MergedElement};

// https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes
// Global attributes are attributes common to all HTML elements; they can be used on all elements, though they may have no effect on some elements.
// Global attributes may be specified on all HTML elements, even those not specified in the standard.

const GLOBAL_ATTRIBUTE_NAMES: &[&str] = &[
    "accesskey", 
    "autocapitalize", 
    "autofocus", 
    "class", 
    "contenteditable", 
    // "contextmenu", //  "Deprecated", 
    // "data-*", XX  https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/data-*
    "dir", 
    "draggable", 
    "enterkeyhint", 
    "exportparts", // Experimental
    "hidden", 
    "id", 
    "inert", 
    "inputmode", 
    "is", 
    "itemid", 
    "itemprop", 
    "itemref", 
    "itemscope", 
    "itemtype", 
    "lang", 
    "nonce", 
    "part", 
    "popover", 
    "role", 
    "slot", 
    "spellcheck", 
    "style", 
    "tabindex", 
    "title", 
    "translate", 
    "virtualkeyboardpolicy", 
    // The ARIA role attribute and the multiple aria-* states and
    // properties, used for ensuring accessibility.
    "role", 
    ];

const EVENT_HANDLER_ATTRIBUTE_NAMES: &[&str] = &[
    "onabort", "onautocomplete", "onautocompleteerror", "onblur", "oncancel", "oncanplay", "oncanplaythrough", "onchange", "onclick", "onclose", "oncontextmenu", "oncuechange", "ondblclick", "ondrag", "ondragend", "ondragenter", "ondragleave", "ondragover", "ondragstart", "ondrop", "ondurationchange", "onemptied", "onended", "onerror", "onfocus", "oninput", "oninvalid", "onkeydown", "onkeypress", "onkeyup", "onload", "onloadeddata", "onloadedmetadata", "onloadstart", "onmousedown", "onmouseenter", "onmouseleave", "onmousemove", "onmouseout", "onmouseover", "onmouseup", "onmousewheel", "onpause", "onplay", "onplaying", "onprogress", "onratechange", "onreset", "onresize", "onscroll", "onseeked", "onseeking", "onselect", "onshow", "onsort", "onstalled", "onsubmit", "onsuspend", "ontimeupdate", "ontoggle", "onvolumechange", "onwaiting"
];


#[derive(Debug)]
pub struct Attribute {
    // pub name: KString, -- already known as key in HashMap
    pub description: KString,
    pub ty: AttributeType,
}

#[derive(Debug)]
pub struct ElementMeta {
    pub tag_name: KString,
    //pub description: KString,-- ah, don't have!
    pub has_global_attributes: bool,
    pub has_closing_tag: bool,
    pub attributes: HashMap<KString, Attribute>,
    pub allows_child_text: bool,
    pub child_elements: HashSet<KString>,
}

impl PartialEq for ElementMeta {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || self.tag_name == other.tag_name
    }
}

impl Eq for ElementMeta {}

#[derive(Debug)]
pub struct MetaDb {
    pub global_attribute_names: HashSet<KString>,
    // ^ currently with no further info about them!
    pub elementmeta: HashMap<KString, ElementMeta>,
}


fn read_types(path: &Path) -> Result<MergedElement> {
    Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?)
}

fn read_types_db() -> Result<HashMap<KString, MergedElement>> {
    let mut m = HashMap::new();
    for entry in read_dir("resources/merged/elements/")? {
        let path = entry?.path();
        (|| -> Result<()> {
            let filename = path.file_name().ok_or_else(
                || anyhow!("path has no file_name: {:?}", path))?;
            let name = filename.to_string_lossy();
            let elementname = name.strip_suffix(".json").ok_or_else(
                || anyhow!("path is missing .json suffix: {:?}", path))?;

            m.insert(KString::from_ref(elementname), read_types(&path)?);
            Ok(())
        })().with_context(
            || anyhow!("path {:?}", path))?;
    }
    Ok(m)
}

// trait ToFunction<K, V, F: Fn(&K) -> Option<&V>> {
//     fn to_function(self) -> F;
// }

// impl<K, V, F: Fn(&K) -> Option<&V>> ToFunction<K, V, F> for HashMap<K, V> {
//     fn to_function(self) -> F {
//     }
// }


pub fn read_meta_db() -> Result<MetaDb> {
    let empty_kstring = KString::from_ref("");

    let ts = read_types_db()?;

    let mut tag_name_by_struct_name: HashMap<KString, KString> = HashMap::new();
    for (tag_name, elt) in &ts {
        tag_name_by_struct_name.insert(elt.struct_name.clone(),
                                       tag_name.clone()); 
    }

    let mut elementmeta = HashMap::new();
    for (k, v) in ts {
        // dbg!((&k, &v));
        let mut attributes = HashMap::new();
        for att in v.attributes {
            attributes.insert(att.name, Attribute {
                description: att.description,
                ty: att.ty
            });
        }

        let mut child_elements: HashSet<KString> =
            v.permitted_child_elements.iter().map(
                |k| {
                    if let Some(tn) = tag_name_by_struct_name.get(k) {
                        tn.clone()
                    } else {
                        if k == "Text" {
                            empty_kstring.clone()
                        } else {
                            panic!("unknown permitted child element name {:?}", k)
                        }
                    }
                }).collect();
        let allows_child_text =
            child_elements.take(&empty_kstring).is_some();
        
        elementmeta.insert(k, ElementMeta {
            tag_name: v.tag_name,
            has_global_attributes: v.has_global_attributes,
            has_closing_tag: v.has_closing_tag,
            attributes,
            allows_child_text,
            child_elements,
        });
    }

    let mut global_attribute_names: HashSet<KString> = HashSet::new();
    for n in GLOBAL_ATTRIBUTE_NAMES {
        global_attribute_names.insert(KString::from_static(n));
    }
    for n in EVENT_HANDLER_ATTRIBUTE_NAMES {
        global_attribute_names.insert(KString::from_static(n));
    }
    
    Ok(MetaDb {
        global_attribute_names,
        elementmeta,
    })
}

