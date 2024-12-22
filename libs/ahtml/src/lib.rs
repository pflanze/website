//! Html dom abstraction, with runtime typing.

pub mod myfrom;
pub mod arc_util;
pub mod util;
pub mod allocator;
pub mod flat;
pub mod more_vec;

use std::{cell::RefMut,
          io::Write};
pub use allocator::{HtmlAllocator, AllocatorPool, AId, Node, ASlice, Element,
                    AllocatorType, SerHtmlFrag, ToASlice, AVec};
use kstring::KString;
use anyhow::{Result, bail};
use lazy_static::lazy_static;
use ahtml_html::meta::{MetaDb, ElementMeta, read_meta_db};

use crate::myfrom::MyFrom;

pub const NBSP: &str = "\u{00A0}";

// https://www.w3.org/International/questions/qa-byte-order-mark#problems
const BOM: &str = "\u{FEFF}";
#[cfg(test)]
#[test]
fn t_file_encoding() {
    assert_eq!(BOM.as_bytes(), &[0xEF, 0xBB, 0xBF]);
}

const DOCTYPE: &str = "<!DOCTYPE html>\n";

pub trait Print {
    /// Print serialized HTML.
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator)
                           -> Result<()>;

    /// Print plain text, completely *ignoring* HTML markup. Can
    /// currently only give an error if encountering preserialized
    /// HTML.
    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator)
                   -> Result<()>;

    fn to_html_fragment_string(&self, allocator: &HtmlAllocator) -> Result<String> {
        let mut s = Vec::new();
        self.print_html_fragment(&mut s, allocator)?;
        Ok(unsafe {
            // Safe because v was filled from bytes derived from
            // String/str values and byte string literals (typed in via
            // Emacs) that were simply concatenated together.
            String::from_utf8_unchecked(s)
        })
    }

    fn to_plain_string(&self, allocator: &HtmlAllocator) -> Result<String> {
        let mut s = String::new();
        self.print_plain(&mut s, allocator)?;
        Ok(s)
    }

    // fn to_kstring -- don't provide this as it will be confused for
    // preserialized HTML and that currently only supports elements,
    // not slices.
}

impl Print for AId<Node> {
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator)
                  -> Result<()> {
        let node = allocator.get_node(*self).expect("id should resolve: {self:?}");
        node.print_html_fragment(out, allocator)
    }

    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator)
                   -> Result<()> {
        let node = allocator.get_node(*self).expect("id should resolve: {self:?}");
        node.print_plain(out, allocator)
    }
}



lazy_static!{
    pub static ref METADB: MetaDb = read_meta_db().unwrap();
}

impl AllocatorPool {
    pub fn new(max_id: u32, verify: bool) -> AllocatorPool {
        Self::new_with_metadb(max_id,
                              if verify {
                                  Some(&*METADB)
                              } else {
                                  None
                              })
    }
}

impl HtmlAllocator {
    pub fn new(max_id: u32) -> Self {
        Self::new_with_metadb(max_id, Some(&*METADB))
    }
}


fn ks<T>(s: T) -> KString
where KString: MyFrom<T>
{
    KString::myfrom(s)
}

pub fn att<T, U>(key: T, val: U) -> Option<(KString, KString)>
    where KString: MyFrom<T> + MyFrom<U>
{
    Some((ks(key), ks(val)))
}

pub fn opt_att<T, U>(key: T, val: Option<U>) -> Option<(KString, KString)>
    where KString: MyFrom<T> + MyFrom<U>
{
    val.map(|val| (ks(key), ks(val)))
}


impl<T> ToASlice<T> for ASlice<T> {
    fn to_aslice(self, _allocator: &HtmlAllocator) -> Result<ASlice<T>> {
        Ok(self)
    }
}
impl<T> ToASlice<T> for &ASlice<T> {
    fn to_aslice(self, _allocator: &HtmlAllocator) -> Result<ASlice<T>> {
        Ok(*self)
    }
}
impl<'a, T: AllocatorType> ToASlice<T> for AVec<'a, T> {
    fn to_aslice(self, _allocator: &HtmlAllocator) -> Result<ASlice<T>> {
        Ok(self.as_slice())
    }
}

impl ToASlice<Node> for AId<Node> {
    fn to_aslice(self, html: &HtmlAllocator) -> Result<ASlice<Node>> {
        let mut vec = html.new_vec();
        vec.push(self)?;
        Ok(vec.as_slice())
    }
}

// Take ownership of an array (best syntax, and allows to avoid the
// need for swap), version for attributes:

// Disabled for now, because with stable Rust we can't resolve the
// ambiguity with empty arrays without explicit typing
// impl<'a, const N: usize> ToASlice<(KString, KString)> for [(KString, KString); N] {
//     fn to_aslice(
//         self,
//         allocator: &Allocator
//     ) -> Result<ASlice<(KString, KString)>>
//     {
//         // Instantiated for every length, need to keep this short! --
//         // except if we want to avoid the swap, there is no length
//         // independent way to do it, hence have to be fat,
//         // bummer. FUTURE: optimize via unsafe memcpy (if the type
//         // isn't pinned etc.).
//         let mut vec = allocator.new_vec();
//         for val in self {
//             let id_ = allocator.new_attribute(val)?;
//             vec.push(id_)?;
//         }
//         Ok(vec.as_slice())
//     }
// }

// Same for values returned by `att` and `opt_att`:
impl<'a, const N: usize> ToASlice<(KString, KString)> for [Option<(KString, KString)>; N] {
    fn to_aslice(
        self,
        allocator: &HtmlAllocator
    ) -> Result<ASlice<(KString, KString)>>
    {
        let mut vec = allocator.new_vec();
        for opt_val in self {
            if let Some(val) = opt_val {
                let id_ = allocator.new_attribute(val)?;
                vec.push(id_)?;
            }
        }
        Ok(vec.as_slice())
    }
}

impl<const N: usize> ToASlice<Node> for [AId<Node>; N] {
    fn to_aslice(self, allocator: &HtmlAllocator) -> Result<ASlice<Node>> {
        // Instantiated for every length, need to keep this short! --
        // except if we want to avoid the swap, there is no length
        // independent way to do it, hence have to be fat,
        // bummer. FUTURE: optimize via unsafe memcpy (if the type
        // isn't pinned etc.).
        let mut vec = allocator.new_vec();
        for val in self {
            vec.push(val)?;
        }
        Ok(vec.as_slice())
    }
}



impl HtmlAllocator {
    /// `bytes` must represent proper UTF-8,
    /// e.g. string.as_bytes(). The resulting reference must be
    /// dropped before calling html_escape again, or there will be a
    /// panic.
    pub fn html_escape(&self, bytes: &[u8]) -> RefMut<Vec<u8>> {
        let mut bufref = self.html_escape_tmp.borrow_mut();
        let append = |buf: &mut Vec<u8>, bstr: &[u8]| {
            // XX wanted to use copy_from_slice. But how to reserve
            // space for it efficiently?
            buf.extend(bstr.iter());
        };
        let buf = &mut *bufref;
        buf.clear();
        for b in bytes {
            match b {
                b'&' => append(buf, b"&amp;"),
                b'<' => append(buf, b"&lt;"),
                b'>' => append(buf, b"&gt;"),
                b'"' => append(buf, b"&quot;"),
                b'\'' => append(buf, b"&#39;"),
                _=> buf.push(*b)
            }
        }
        bufref
    }

    pub fn print_html_fragment(&self, id_: AId<Node>, out: &mut impl Write) -> Result<()> {
        let noderef = self.get_node(id_).expect(
            // (Why does this return a Result even ? Aha, for
            // invalid dynamic borrow. Should this be changed to panic,
            // too?)
            "invalid generation/allocator_id leads to panic, hence this should \
             always resolve");
        match &*noderef {
            Node::Element(_) => (),
            Node::String(_) => {
                // eprintln!("toplevel print_html: Warning: printing of a \
                //            Node::String")
            }
            Node::Preserialized(_) =>
                eprintln!("toplevel print_html: Warning: printing of a \
                           Node::Preserialized"),
            Node::None => {},
        }
        noderef.print_html_fragment(out, self)
    }

    pub fn print_html_document(&self, id_: AId<Node>, out: &mut impl Write) -> Result<()> {
        // Add a byte-order mark (BOM) to make sure the output is read
        // correctly from files, too (e.g. by Safari).
        out.write_all(BOM.as_bytes())?;
        out.write_all(DOCTYPE.as_bytes())?;
        self.print_html_fragment(id_, out)
    }

    pub fn to_html_string(&self, id: AId<Node>, want_doctype: bool) -> String {
        let mut v = Vec::new();
        if want_doctype {
            self.print_html_document(id, &mut v)
        } else {
            self.print_html_fragment(id, &mut v)
        }.expect("no I/O errors can happen");

        // Safe because v was filled from bytes derived from
        // String/str values and byte string literals (typed in via
        // Emacs) that were simply concatenated together.
        unsafe { String::from_utf8_unchecked(v) }
    }

    /// Returns an error if id doesn't refer to an Element Node.
    pub fn preserialize(&self, id: AId<Node>) -> Result<SerHtmlFrag> {
        let meta = {
            let noderef = self.get_node(id).expect(
                // (Why does this return a Result even ? Aha, for
                // invalid dynamic borrow. Should this be changed to panic,
                // too?)
                "invalid generation/allocator_id leads to panic, hence this should \
                 always resolve");
            let n = &*noderef;
            match n {
                Node::Element(e) => e.meta,
                _ => bail!("can only preserialize element nodes")
            }
        };
        let s = self.to_html_string(id, false);
        Ok(SerHtmlFrag {
            meta,
            kstring: KString::from_string(s)
        })
    }

    // 2x partial copy-paste

    pub fn print_plain(&self, id: AId<Node>, out: &mut String) -> Result<()> {
        let noderef = self.get_node(id).expect(
            // (Why does this return a Result even ? Aha, for
            // invalid dynamic borrow. Should this be changed to panic,
            // too?)
            "invalid generation/allocator_id leads to panic, hence this should \
             always resolve");
        match &*noderef {
            Node::Element(_) => (),
            Node::String(_) => {
                // eprintln!("toplevel print_plain: Warning: printing of a \
                //            Node::String")
            }
            Node::Preserialized(_) =>
            // XX eh, that won't work anyway, error later on?
                eprintln!("toplevel print_plain: Warning: printing of a \
                           Node::Preserialized"),
            Node::None => {},
        }
        noderef.print_plain(out, self)
    }

    /// If you need this to strip html and use the result as AId, be
    /// sure to use `to_plain_string_aid` instead, as that optimizes
    /// the case of `id` already representing a string.
    pub fn to_plain_string(&self, id: AId<Node>) -> Result<KString> {
        let mut v = String::new();
        self.print_plain(id, &mut v)?;
        Ok(KString::from_string(v))
    }

    /// Like `to_plain_string` but returns a string node (or empty
    /// node if the input is empty) and optimizes (and silences) the
    /// case where `id` already represents a string.
    pub fn to_plain_string_aid(&self, id: AId<Node>) -> Result<AId<Node>> {
        let noderef = self.get_node(id).expect(
            // (Why does this return a Result even ? Aha, for
            // invalid dynamic borrow. Should this be changed to panic,
            // too?)
            "invalid generation/allocator_id leads to panic, hence this should \
             always resolve");
        match &*noderef {
            Node::Element(_) => {
                drop(noderef); // free up borrow!
                let mut v = String::new();
                self.print_plain(id, &mut v)?;
                self.string(v)
            }
            Node::String(_) => Ok(id),
            // OK to give an error right away? *Would* error out
            // anyway on print_plain ('though'), right?
            Node::Preserialized(_) =>
                bail!("can't currently strip markup from preserialized HTML"),
            Node::None => Ok(id), // XX is this OK or do we promise to return a string node?
        }
    }
}

include!("../includes/ahtml_elements_include.rs");


impl<T: AllocatorType> Print for ASlice<T> {
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator)
                  -> Result<()> {
        for node in self.iter_node(allocator) {
            node.print_html_fragment(out, allocator)?;
        }
        Ok(())
    }

    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator)
                   -> Result<()> {
        for node in self.iter_node(allocator) {
            node.print_plain(out, allocator)?;
        }
        Ok(())
    }
}


impl Print for (KString, KString) {
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator)
             -> Result<()> {
        out.write_all(self.0.as_bytes())?; // XX no escape ever needed?
        out.write_all(b"=\"")?;
        out.write_all(&allocator.html_escape(self.1.as_bytes()))?;
        out.write_all(b"\"")?;
        Ok(())
    }

    fn print_plain(&self, _out: &mut String, _allocator: &HtmlAllocator) -> Result<()> {
        panic!("attributes are never printed in print_plain for Node:s")
    }
}


impl Print for Node {
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator) -> Result<()>
    {
        Ok(match self {
            Node::Element(e) => e.print_html_fragment(out, allocator)?,
            Node::String(s) => out.write_all(&allocator.html_escape(s.as_bytes()))?,
            Node::Preserialized(ser) =>
                out.write_all(ser.as_str().as_bytes())?,
            Node::None => (),
        })
    }
    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator) -> Result<()>
    {
        match self {
            Node::Element(e) => e.print_plain(out, allocator),
            Node::String(s) => Ok(out.push_str(s.as_str())),
            Node::Preserialized(_) =>
                // would require re-parsing
                bail!("print_plain: cannot (currently) print pre-serialized HTML \
                       as plain text"),
            Node::None => Ok(()),
        }
    }
}


impl Print for Element {
    fn print_html_fragment(&self, out: &mut impl Write, allocator: &HtmlAllocator)
             -> Result<()>
    {
        let meta = self.meta;
        // meta.has_global_attributes XX ? only for verification?
        out.write_all(b"<")?;
        out.write_all(meta.tag_name.as_bytes())?;
        for att in self.attr.iter_att(allocator) {
            out.write_all(b" ")?;
            att.print_html_fragment(out, allocator)?;
        }
        out.write_all(b">")?;
        self.body.print_html_fragment(out, allocator)?;
        if meta.has_closing_tag {
            out.write_all(b"</")?;
            out.write_all(meta.tag_name.as_bytes())?;
            out.write_all(b">")?;
        }
        Ok(())
    }

    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator) -> Result<()> {
        self.body.print_plain(out, allocator)
    }
}


pub trait TryCollectBody {
    fn try_collect_body(&mut self, html: &HtmlAllocator) -> Result<ASlice<Node>>;
}

impl<I: Iterator<Item = Result<AId<Node>>>> TryCollectBody for I {
    fn try_collect_body(&mut self, html: &HtmlAllocator) -> Result<ASlice<Node>> {
        let mut v = html.new_vec::<Node>();
        for item in self {
            v.push(item?)?;
        }
        Ok(v.as_slice())
    }
}


// fn p_ab(attr: &[(KString, KString)], body: &[Node]) -> Element {
    // Element {
    //     meta: &P_META,
    //     attr: Some(Box::new(*attr)),
    //     body: Some(Box::new(*body)),
    // }
// }


// trait HtmlCheck {
//     fn check(&self) -> Result<()>;
// }

// impl HtmlCheck for Node {
//     fn check(&self) -> Result<()> {
//         Ok(())
//     }
// }

