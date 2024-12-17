//! Cleaned up and indexed data for fast DOM correctness verification.

use std::{collections::{HashMap, HashSet},
          path::Path, fs::read_dir, hash::Hash, io::{Write, BufWriter},
          env, str::FromStr, fmt::Display};
use anyhow::{anyhow, Result, Context, bail};
use kstring::KString;
use crate::{types::{AttributeType, MergedElement}, myfrom::MyFrom};

// =============================================================================
// Attributes database

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

// =============================================================================
// Element database representation

// The data is provided as .json files, but we want to include the
// info in the binary statically. We want to use HashSet and HashMap
// though, and then also KString since it may speed up access due to
// data locality over &'static str (untested hypothesis). So we have
// two variants of all datatypes, the Static* and the non-Static ones.

// Data is first read from the json files into the non-Static
// variants, then printed via `PrintStatic` in Static* syntax to an include
// file, which is compiled into the binary. Then at start time, those
// are converted back to the non-Static versions via MyFrom.

trait PrintStatic {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()>;
}


impl PrintStatic for KString {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "{:?}", self.as_str())
    }
}

// Helper wrappers

struct StaticVec<'t, T>(&'t [T]);

impl<T: PrintStatic> PrintStatic for Vec<T> {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticVec(&[\n")?;
        let mut vs = self.iter();
        let mut current = vs.next();
        while let Some(v) = current {
            v.print_static(out)?;
            let next = vs.next();
            if next.is_some() {
                write!(out, ",\n")?
            }
            current = next;
        }
        write!(out, "])\n")
    }
}

impl<'t, T1, T2: MyFrom<&'t T1>>
    MyFrom<&StaticVec<'t, T1>>
    for Vec<T2>
{
    fn myfrom(v: &StaticVec<'t, T1>) -> Self {
        v.0.iter().map(|v| T2::myfrom(v)).collect()
    }
}

struct StaticMap<'t, K, V>(&'t [(K, V)]);

impl<K: PrintStatic + Ord, V: PrintStatic> PrintStatic for HashMap<K, V> {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticMap(&[\n")?;
        let mut vals: Vec<_> = self.iter().collect();
        vals.sort_by_key(|(k, _v)| *k);
        let mut vs = vals.into_iter();
        let mut current = vs.next();
        while let Some((k, v)) = current {
            write!(out, "(")?;
            k.print_static(out)?;
            write!(out, ", ")?;
            v.print_static(out)?;
            write!(out, ")")?;
            let next = vs.next();
            if next.is_some() {
                write!(out, ",\n")?
            }
            current = next;
        }
        write!(out, "])\n")
    }
}

impl<'t, K1, V1, K2: MyFrom<&'t K1> + Hash + Eq, V2: MyFrom<&'t V1>>
    MyFrom<&StaticMap<'t, K1, V1>>
    for HashMap<K2, V2>
{
    fn myfrom(v: &StaticMap<'t, K1, V1>) -> Self {
        v.0.iter().map(|(k, v)| (K2::myfrom(k), V2::myfrom(v))).collect()
    }
}

struct StaticSet<'t, T>(&'t [T]);

impl<'t, T: PrintStatic + Ord> PrintStatic for HashSet<T> {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticSet(&[\n")?;
        let mut vals: Vec<_> = self.iter().collect();
        vals.sort();
        let mut vs = vals.into_iter();
        let mut current = vs.next();
        while let Some(v) = current {
            v.print_static(out)?;
            let next = vs.next();
            if next.is_some() {
                write!(out, ",\n")?
            }
            current = next;
        }
        write!(out, "])\n")
    }
}

impl<'t, T1, T2: MyFrom<&'t T1> + Hash + Eq>
    MyFrom<&StaticSet<'t, T1>>
    for HashSet<T2>
{
    fn myfrom(v: &StaticSet<'t, T1>) -> Self {
        v.0.iter().map(|v| T2::myfrom(v)).collect()
    }
}


// AttributeType: see types.rs

enum StaticAttributeType<'t> {
    Bool,
    KString,
    Integer,
    Float,
    Identifier(&'t str),
    Enumerable(StaticVec<'t, &'t str>),
}

impl PrintStatic for AttributeType {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        let mut pr = |s| {
            write!(out, "StaticAttributeType::{s}")
        };
        match self {
            AttributeType::Bool => pr("Bool"),
            AttributeType::KString => pr("KString"),
            AttributeType::Integer => pr("Integer"),
            AttributeType::Float => pr("Float"),
            AttributeType::Identifier(s) =>
                write!(out, "StaticAttributeType::Identifier({:?})",
                       s.as_str()),
            AttributeType::Enumerable(vec) => {
                write!(out, "StaticAttributeType::Enumerable(")?;
                vec.print_static(out)?;
                write!(out, ")")
            }
        }
    }
}

impl<'t>
    MyFrom<&StaticAttributeType<'t>>
    for AttributeType
{
    fn myfrom(s: &StaticAttributeType<'t>) -> Self {
        match s {
            StaticAttributeType::Bool => AttributeType::Bool,
            StaticAttributeType::KString => AttributeType::KString,
            StaticAttributeType::Integer => AttributeType::Integer,
            StaticAttributeType::Float => AttributeType::Float,
            StaticAttributeType::Identifier(v) => AttributeType::Identifier(KString::myfrom(v)),
            StaticAttributeType::Enumerable(v) => AttributeType::Enumerable(Vec::myfrom(v)),
        }
    }
}



#[derive(Debug)]
pub struct Attribute {
    // pub name: KString, -- already known as key in HashMap
    pub description: KString,
    pub ty: AttributeType,
}

struct StaticAttribute<'t> {
    // pub name: KString, -- already known as key in HashMap
    pub description: &'t str,
    pub ty: StaticAttributeType<'t>,
}

impl PrintStatic for Attribute {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticAttribute {{\n    description: {:?},\n    ty: ",
               self.description.as_str())?;
        self.ty.print_static(out)?;
        write!(out, "}}\n")
    }
}

impl<'t>
    MyFrom<&StaticAttribute<'t>>
    for Attribute
{
    fn myfrom(s: &StaticAttribute<'t>) -> Self {
        Attribute {
            description: KString::myfrom(s.description),
            ty: AttributeType::myfrom(&s.ty),
        }
    }
}


#[derive(Debug)]
pub struct ElementMeta {
    pub tag_name: KString,
    pub has_global_attributes: bool,
    pub has_closing_tag: bool,
    pub attributes: HashMap<KString, Attribute>,
    pub allows_child_text: bool,
    pub child_elements: HashSet<KString>,
}

struct StaticElementMeta<'t> {
    pub tag_name: &'t str,
    pub has_global_attributes: bool,
    pub has_closing_tag: bool,
    pub attributes: StaticMap<'t, &'t str, StaticAttribute<'t>>,
    pub allows_child_text: bool,
    pub child_elements: StaticSet<'t, &'t str>,
}

impl PrintStatic for ElementMeta {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticElementMeta {{\n")?;
        write!(out, "    tag_name: {:?}\n", self.tag_name.as_str())?;
        write!(out, ",\n    has_global_attributes: {:?}\n", self.has_global_attributes)?;
        write!(out, ",\n    has_closing_tag: {:?}\n", self.has_closing_tag)?;
        write!(out, ",\n    attributes: ")?;
        self.attributes.print_static(out)?;
        write!(out, ",\n    allows_child_text: {:?}\n", self.allows_child_text)?;
        write!(out, ",\n    child_elements: ")?;
        self.child_elements.print_static(out)?;
        write!(out, "}}\n")
    }
}

impl<'t>
    MyFrom<&StaticElementMeta<'t>>
    for ElementMeta
{
    fn myfrom(s: &StaticElementMeta<'t>) -> Self {
        ElementMeta {
            tag_name: KString::myfrom(&s.tag_name),
            has_global_attributes: s.has_global_attributes,
            has_closing_tag: s.has_closing_tag,
            attributes: HashMap::myfrom(&s.attributes),
            allows_child_text: s.allows_child_text,
            child_elements: HashSet::myfrom(&s.child_elements)
        }
    }
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
    pub elementmeta: HashMap<KString, ElementMeta>,
}

struct StaticMetaDb<'t> {
    pub global_attribute_names: StaticSet<'t, &'t str>,
    pub elementmeta: StaticMap<'t, &'t str, StaticElementMeta<'t>>,
}

impl PrintStatic for MetaDb {
    fn print_static<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        write!(out, "StaticMetaDb {{\n")?;
        write!(out, "    global_attribute_names: ")?;
        self.global_attribute_names.print_static(out)?;
        write!(out, ",\n    elementmeta: ")?;
        self.elementmeta.print_static(out)?;
        write!(out, "}}\n")
    }
}

impl<'t>
    MyFrom<&StaticMetaDb<'t>>
    for MetaDb
{
    fn myfrom(s: &StaticMetaDb<'t>) -> Self {
        MetaDb {
            global_attribute_names: HashSet::myfrom(&s.global_attribute_names),
            elementmeta: HashMap::myfrom(&s.elementmeta)
        }
    }
}


fn read_types(path: &Path) -> Result<MergedElement> {
    Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?)
}

fn read_types_db(merged_elements_dir: &Path) -> Result<HashMap<KString, MergedElement>> {
    (|| -> Result<HashMap<KString, MergedElement>> {
            let mut m = HashMap::new();
            for entry in read_dir(merged_elements_dir)
                .with_context(|| anyhow!("reading directory {merged_elements_dir:?}"))?
            {
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
    })().with_context(|| anyhow!("reading types db from {merged_elements_dir:?}"))
}

// trait ToFunction<K, V, F: Fn(&K) -> Option<&V>> {
//     fn to_function(self) -> F;
// }

// impl<K, V, F: Fn(&K) -> Option<&V>> ToFunction<K, V, F> for HashMap<K, V> {
//     fn to_function(self) -> F {
//     }
// }


pub fn read_meta_db_from_json(merged_elements_dir: &Path) -> Result<MetaDb> {
    let empty_kstring = KString::from_ref("");

    let ts = read_types_db(merged_elements_dir)?;

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


// once again, XX move to lib
fn opt_get_env<T: FromStr>(varname: &str) -> Result<Option<T>>
    where T::Err: Display
{
    match env::var(varname) {
        Ok(s) => {
            // dbg!(&s);
            Ok(Some(s.parse().map_err(
                |e| anyhow!("could not parse {varname:?} env var with contents {s:?}: {e}"))?))
        },
        Err(e) => match e {
            env::VarError::NotPresent => Ok(None),
            env::VarError::NotUnicode(_) => bail!("could not decode {varname:?} env var: {e}")
        }
    }
}

fn get_env_bool(varname: &str) -> Result<bool> {
    Ok(opt_get_env(varname)?.unwrap_or(false))
}

include!("../includes/static_meta_db.rs");

pub fn read_meta_db() -> Result<MetaDb> {
    let debug = get_env_bool("HTML_META_DEBUG")?;
    if let Some(dir) = opt_get_env::<String>("HTML_READ_META_DB_FROM_JSON_DIR")? {
        if debug { eprintln!("reading meta db from json") };
        let metadb = read_meta_db_from_json(dir.as_ref())?;
        // XX HACK
        if let Some(path) = opt_get_env::<String>("WRITE_STATIC_META_DB_RS_PATH")? {
            if debug { eprintln!("rewriting {path:?} from meta db from json..") };
            let mut out = BufWriter::new(
                std::fs::File::create(path.as_str())
                    .with_context(|| anyhow!("creating file {path:?} for writing"))?);
            (|| -> Result<()> {
                let out = &mut out;
                write!(out, "
// This file was auto-generated by meta.rs from the ahtml_html crate,
// using the meta database at {path:?}

const STATIC_META_DB: StaticMetaDb = ")?;
                metadb.print_static(out)?;
                write!(out, ";\n")?;
                out.flush()?;
                Ok(())
            })().with_context(|| anyhow!("writing to {path:?}"))?;
            if debug { eprintln!("rewriting {path:?} from meta db from json..done.") };
        }
        Ok(metadb)
    } else {
        if debug { eprintln!("reading meta db from static") };
        Ok(MetaDb::myfrom(&STATIC_META_DB))
    }
}
