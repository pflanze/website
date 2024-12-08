//! Html dom abstraction, with runtime typing.

pub mod myfrom;
pub mod arc_util;

use std::{marker::PhantomData,
          cell::{RefCell, Ref, RefMut},
          cmp::max,
          io::Write,
          sync::{Mutex, Arc, atomic::AtomicBool},
          collections::HashSet};
use backtrace::Backtrace;
use kstring::KString;
use anyhow::{Result, bail};
use lazy_static::lazy_static;
use ahtml_html::meta::{MetaDb, ElementMeta, read_meta_db};

use chj_util::{warn, u24::{U24, U24MAX}, partialbacktrace::PartialBacktrace};

use crate::myfrom::MyFrom;
use crate::arc_util::IntoArc;

// once again
fn all_whitespace(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_whitespace())
}

pub enum Flat<T> {
    None,
    One(AId<T>),
    Two(AId<T>, AId<T>),
    Slice(ASlice<T>)
}

pub trait Print {
    /// Print serialized HTML.
    fn print_html(&self, out: &mut impl Write, allocator: &HtmlAllocator)
                  -> Result<()>;
    /// Print plain text, completely *ignoring* HTML markup. Can
    /// currently only give an error if encountering preserialized
    /// HTML.
    fn print_plain(&self, out: &mut String, allocator: &HtmlAllocator)
                   -> Result<()>;

    fn to_string(&self, allocator: &HtmlAllocator) -> Result<String> {
        let mut s = String::new();
        self.print_plain(&mut s, allocator)?;
        Ok(s)
    }

    // fn to_kstring -- don't provide this as it will be confused for
    // preserialized HTML and that currently only supports elements,
    // not slices.
}


lazy_static!{
    pub static ref METADB: MetaDb = read_meta_db().unwrap();
}

#[derive(Debug)]
pub enum AllocKind {
    Att,
    Node,
    Id,
}

pub trait AllocatorType {
    fn allockind() -> AllocKind;
}

impl AllocatorType for (KString, KString) {
    fn allockind() -> AllocKind {
        AllocKind::Att
    }
}

impl AllocatorType for Node {
    fn allockind() -> AllocKind {
        AllocKind::Node
    }
}

impl<T> AllocatorType for AId<T> {
    fn allockind() -> AllocKind {
        AllocKind::Id
    }
}
// ^ and again not id target specific.  Or should I have different
// AllocKind ones.


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


pub trait ToASlice<T> {
    fn to_aslice(self, allocator: &HtmlAllocator) -> Result<ASlice<T>>;
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
/// For general passing of n values as an ASlice from multiple
/// branches of code, where an owned array doesn't work because of
/// the different types.
impl<T: AllocatorType> ToASlice<T> for Flat<T> {
    fn to_aslice(self, allocator: &HtmlAllocator) -> Result<ASlice<T>> {
        match self {
            Flat::None => Ok(allocator.empty_slice()),
            Flat::One(a) => {
                let mut v = allocator.new_vec();
                v.push(a)?;
                Ok(v.as_slice())
            }
            Flat::Two(a, b) => {
                let mut v = allocator.new_vec();
                v.push(a)?;
                v.push(b)?;
                Ok(v.as_slice())
            }
            Flat::Slice(sl) => Ok(sl),
        }
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


pub struct AllocatorPool {
    max_id: u32, // See Allocator
    metadb: Option<&'static MetaDb>, // See Allocator
    allocators: Mutex<Vec<HtmlAllocator>>,
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
    pub fn new_with_metadb(
        max_id: u32, metadb: Option<&'static MetaDb>
    ) -> AllocatorPool {
        AllocatorPool {
            max_id,
            metadb,
            allocators: Mutex::new(Vec::new())
        }
    }
    pub fn get<'p>(&'p self) -> AllocatorGuard<'p>
    {
        let mut l = self.allocators.lock().unwrap();
        let a = l.pop();
        AllocatorGuard {
            pool: self,
            _allocator: a
        }
    }
}
pub struct AllocatorGuard<'p>
{
    pool: &'p AllocatorPool,
    _allocator: Option<HtmlAllocator>
}
impl<'p> AllocatorGuard<'p> {
    pub fn allocator(&mut self) -> &HtmlAllocator {
        // Safe because the lifetime 'a is passed on to AId, which are
        // valid for the storage. And when drop() is called on
        // AllocatorGuard, none of them exist anymore outside (also
        // drop calls .clear() so none survive inside either).

        // I.e. it is safe to use 'a for further AId allocations. Thus
        // it's correct to use 'a as the lifetime parameter to the
        // existing Allocator, as made visible here to the user (with
        // lifetime 'a, thus matches).

        if self._allocator.is_none() {
            // eprintln!("allocating a new Allocator");
            self._allocator = Some(HtmlAllocator::new_with_metadb(
                self.pool.max_id,
                self.pool.metadb.clone(),
            ));
        }
        self._allocator.as_mut().unwrap()
    }
}
impl<'p> Drop for AllocatorGuard<'p> {
    fn drop(&mut self) {
        let mut l = self.pool.allocators.lock().unwrap();
        let mut a = self._allocator.take().unwrap();
        if a.regionid.generation < 20 {
            a.clear();
            // Insert it back into the pool:
            l.push(a);
        }
    }
}

pub struct HtmlAllocator {
    // For dynamic verification of AId:s, also the generation counter
    // is used to stop reusing the allocator at some point to free up
    // unused memory.
    regionid: RegionId,
    // If present, DOM structure validation is done (at runtime):
    metadb: Option<&'static MetaDb>,
    // Storage for attributes:
    atts: RefCell<Vec<Option<(KString, KString)>>>,
    // Storage for nodes:
    nodes: RefCell<Vec<Option<Node>>>,
    // Storage for references to attributes and nodes:
    ids: RefCell<Vec<u32>>, // for attribute or Node, depending on slot
    // Maximum length of any of the storage types above:
    max_id: u32,
    // Temporary storage for serialisation:
    html_escape_tmp: RefCell<Vec<u8>>,
}

lazy_static!{
    static ref NEXT_ALLOCATOR_ID: Mutex<u32> = Mutex::new(0);
}
fn next_allocator_id() -> U24 {
    // replace with atomic inc?
    let mut guard = NEXT_ALLOCATOR_ID.lock().unwrap();
    let id = *guard;
    *guard =
        if id < U24MAX {
            id + 1
        } else {
            0
        };
    U24::new(id)
}

pub static AHTML_TRACE: AtomicBool = AtomicBool::new(false);

impl HtmlAllocator {
    pub fn new(max_id: u32) -> Self {
        Self::new_with_metadb(max_id, Some(&*METADB))
    }
    pub fn new_with_metadb(max_id: u32, metadb: Option<&'static MetaDb>) -> Self {
        HtmlAllocator {
            regionid: RegionId {
                allocator_id: next_allocator_id(),
                generation: 0,
            },
            atts: RefCell::new(Vec::new()),
            nodes: RefCell::new(Vec::new()),
            ids: RefCell::new(Vec::new()),
            max_id,
            metadb,
            html_escape_tmp: RefCell::new(Vec::new()),
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.atts.borrow_mut().clear();
        self.nodes.borrow_mut().clear();
        self.ids.borrow_mut().clear();
        self.regionid.generation += 1;
    }
    
    pub fn regionid(&self) -> RegionId {
        self.regionid
    }
    pub fn assert_regionid(&self, rid: RegionId) {
        if rid != self.regionid {
            panic!("regionid mismatch")
        }
    }

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

    pub fn print_html(&self, id_: AId<Node>, out: &mut impl Write) -> Result<()> {
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
        noderef.print_html(out, self)
    }

    fn _to_html_string(&self, id: AId<Node>, want_doctype: bool) -> (Ref<Node>, String) {
        let noderef = self.get_node(id).expect(
            // (Why does this return a Result even ? Aha, for
            // invalid dynamic borrow. Should this be changed to panic,
            // too?)
            "invalid generation/allocator_id leads to panic, hence this should \
             always resolve");
        let mut v = Vec::new();
        if want_doctype {
            write!(&mut v, "<!DOCTYPE html>\n").unwrap();
        }
        noderef.print_html(&mut v, self).expect("no I/O errors can happen");
        // Safe because v was filled from bytes derived from
        // String/str values and byte string literals (typed in via
        // Emacs) that were simply concatenated together.
        let s = unsafe { String::from_utf8_unchecked(v) };
        (noderef, s)
    }

    /// Returns an error if id doesn't refer to an Element Node.
    pub fn preserialize(&self, id: AId<Node>) -> Result<SerHtmlFrag> {
        let (noderef, s) = self._to_html_string(id, false);
        let n = &*noderef;
        let meta = match n {
            Node::Element(e) => e.meta,
            _ => bail!("can only preserialize element nodes")
        };
        Ok(SerHtmlFrag {
            meta,
            kstring: KString::from_string(s)
        })
    }

    pub fn to_html_string(&self, id: AId<Node>, want_doctype: bool) -> String {
        let (_noderef, s) = self._to_html_string(id, want_doctype);
        s
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


    // fn set<T: AllocatorType>(&self, id_: AId<&'a T>, val: T) {
    //     match T::allockind() {
    //         AllocKind::Att => self.atts.borrow_mut()[id_.0 as usize] = Some(val),
    //         AllocKind::Node => (),
    //         AllocKind::Id => (),
    //     }
    // }

    fn id_to_index<T>(&self, id: AId<T>) -> usize {
        self.id_to_bare(id) as usize
    }

    fn id_to_bare<T>(&self, id: AId<T>) -> u32 {
        if self.regionid == id.regionid {
            id.id
        } else {
            panic!("AId with incompatible RegionId used: expected {:?}, got {:?}",
                   self.regionid, id.regionid);
        }
    }

    // first AId is for our own position, second one for position in target T.
    // Now why is first one not maybe AId<AId<T>>?
    fn set_id<T: AllocatorType>(&self, id_bare: u32, val: AId<T>) {
        self.ids.borrow_mut()[id_bare as usize] =
            self.id_to_bare(val);
    }

    pub fn get_node<'a>(&'a self, id: AId<Node>) -> Option<Ref<'a, Node>> {
        let b: Ref<Vec<Option<Node>>> = self.nodes.borrow();
        // let m: Ref<Option<&Option<Node>>> = Ref::map(b, |r| &r.get(0));
        // // ^ odd, & of &r.get.. is not in m any more gll  Ref::map does that deref.
        // let n: Ref<Option<&Option<Node>>> = Ref::map(b, |r| {
        //     &r.get(id_.0 as usize) // ?.as_ref()
        // });
        let n: Option<Ref<Node>> = Ref::filter_map(b, |r| {
            if let Some(v) = r.get(self.id_to_index(id)) {
                if let Some(n) = v {
                    Some(n)
                } else {
                    // "uninitialized" memory
                    None
                }
            } else {
                // id behind end of memory
                None
            }
        }).ok();
        n
    }

    // COPY-PASTE of above with types stripped
    pub fn get_att<'a>(&'a self, id: AId<(KString, KString)>)
                   -> Option<Ref<'a, (KString, KString)>>
    {
        let b = self.atts.borrow();
        let n = Ref::filter_map(b, |r| {
            if let Some(v) = r.get(self.id_to_index(id)) {
                if let Some(n) = v {
                    Some(n)
                } else {
                    // "uninitialized" memory
                    None
                }
            } else {
                // id behind end of memory
                None
            }
        }).ok();
        n
    }

    // For within ids. To get the id, to be of type T.
    pub fn get_id<T>(&self, id_bare: u32) -> Option<AId<T>> {
        // Blindly trusting that the id we are retrieving is pointing
        // to T (XX btw why using a mixed pool for ids, when using
        // separate ones for the objects?)
        self.ids.borrow().get(id_bare as usize).map(
            |id2| AId::new(self.regionid, *id2))
    }
    
    // it's actually a vec of AId, but for T
    pub fn new_vec<'a, T: AllocatorType>(&'a self) -> AVec<'a, T> {
        AVec::new(self)
    }

    pub fn new_vec_with_capacity<'a, T: AllocatorType>(
        &'a self,
        capacity: u32
    ) -> Result<AVec<'a, T>> {
        AVec::new_with_capacity(self, capacity)
    }

    /// But also see element method for more comfort.
    pub fn new_element(
        &self,
        meta: &'static ElementMeta,
        // The slices must be for storage in this
        // Allocator! XX could this be improved?
        attr: ASlice<(KString, KString)>,
        body: ASlice<Node>
    ) -> Result<AId<Node>> {

        // verify
        if let Some(global_meta) = self.metadb {
            {
                let allowed = &meta.attributes;
                for (i, att) in attr.iter_att(self).enumerate() {
                    if global_meta.global_attribute_names.contains(&att.0) {
                        // OK; XX verify attribute value, too, but
                        // don't have the data yet.
                    } else if let Some(_a) = allowed.get(&att.0) {
                        // OK; XX: todo: verify attribute value, too
                    } else {
                        let mut allowednamesset =
                            allowed.keys().map(|k| k.clone()).collect::<HashSet<KString>>();
                        allowednamesset.extend(global_meta.global_attribute_names.iter()
                                               .map(|k| k.clone()));
                        let mut allowednames: Vec<&str> =
                            allowednamesset.iter().map(
                                |v| v.as_str()).collect();
                        allowednames.sort();
                        bail!("invalid attribute #{i} {:?} for element {:?} \
                               (valid: {:?})\n{:?}",
                              att.0.as_str(),
                              meta.tag_name.as_str(),
                              allowednames,
                              Backtrace::new())
                    }
                }
            }
            {
                let allowed = &meta.child_elements;
                for (i, node) in body.iter_node(self).enumerate() {
                    let verify_child_element_meta =
                        |child_meta: &ElementMeta| -> Result<()>
                    {
                        if ! allowed.contains(&child_meta.tag_name) {
                            let mut allowednames: Vec<&str> = allowed.iter().map(
                                |k| k.as_str()).collect();
                            allowednames.sort();
                            bail!("content value #{i}: element {:?} not allowed as \
                                   a child of element {:?}, only: {:?}{}\n{:?}",
                                  child_meta.tag_name.as_str(),
                                  meta.tag_name.as_str(),
                                  allowednames,
                                  if meta.allows_child_text {
                                      " as well as text"
                                  } else {
                                      " (no text)"
                                  },
                                  Backtrace::new())
                        }
                        Ok(())
                    };
                    match &*node {
                        Node::Element(elt) => {
                            verify_child_element_meta(elt.meta)?
                        }
                        Node::String(s) => {
                            if (! meta.allows_child_text) &&
                                (! all_whitespace(s.as_str()))
                            {
                                let mut allowednames: Vec<&str> = allowed.iter().map(
                                    |k| k.as_str()).collect();
                                allowednames.sort();
                                bail!("content value #{i}: text is not allowed as \
                                       a child of element {:?}, only: {:?}\n{:?}",
                                      meta.tag_name.as_str(),
                                      allowednames,
                                      Backtrace::new())
                            }
                        }
                        Node::Preserialized(ser) => {
                            verify_child_element_meta(ser.meta)?
                        }
                        Node::None => {},
                    }
                }
            }
        }

        let mut attr = attr;
        if AHTML_TRACE.load(std::sync::atomic::Ordering::Relaxed) {
            let mut seen_title = false;
            let mut vec = self.new_vec_with_capacity(attr.len + 1)?;
            for id in attr.iter_aid(&self) {
                let r = self.get_att(id).expect("exists because it's in attr");
                if r.0 == "title" {
                    seen_title = true;
                }
                vec.push(id)?;
            }
            let bt_str = PartialBacktrace::new().part_to_string(1, "src/rouille_runner.rs");
            if seen_title {
                warn!("element {:?} already has 'title' attribute, not adding tracing at:\n\
                       {bt_str}",
                      &*meta.tag_name);
            } else {
                vec.push(self.attribute("title", format!("Generated at:\n\
                                                          {bt_str}"))?)?;
            }
            attr = vec.to_aslice(self)?;
        }
        
        let mut nodes= self.nodes.borrow_mut();
        let id_ = nodes.len();
        let newlen = id_ + 1;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        nodes.push(Some(Node::Element(Element {
            meta,
            attr,
            body
        })));
        Ok(AId::new(self.regionid, id_ as u32))
    }

    // XX naming needs work (new_element, element, (add_element), allocate_element).
    pub fn allocate_element(&self, elt: Element) -> Result<AId<Node>> {
        self.new_element(elt.meta, elt.attr, elt.body)
    }

    fn new_string(
        &self,
        s: KString
    ) -> Result<AId<Node>> {
        // much COPY-PASTE always
        let mut nodes= self.nodes.borrow_mut();
        let id_ = nodes.len();
        let newlen = id_ + 1;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        nodes.push(Some(Node::String(s)));
        Ok(AId::new(self.regionid, id_ as u32))
    }
    pub fn empty_node(&self) -> Result<AId<Node>> {
        // much COPY-PASTE always
        let mut nodes= self.nodes.borrow_mut();
        let id_ = nodes.len();
        let newlen = id_ + 1;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        nodes.push(Some(Node::None));
        Ok(AId::new(self.regionid, id_ as u32))
    }

    pub fn new_attribute(
        &self,
        att: (KString, KString)
    ) -> Result<AId<(KString, KString)>>
    {
        let mut atts = self.atts.borrow_mut();
        let id_ = atts.len();
        let newlen = id_ + 1;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        atts.push(Some(att));
        Ok(AId::new(self.regionid, id_ as u32))
    }
    pub fn attribute<K, V>(
        &self,
        key: K,
        val: V
    ) -> Result<AId<(KString, KString)>>
        where KString: MyFrom<K>, KString: MyFrom<V>
    {
        self.new_attribute((KString::myfrom(key), KString::myfrom(val)))
    }

    pub fn preserialized(
        &self,
        val: impl IntoArc<SerHtmlFrag>
    ) -> Result<AId<Node>> {
        // ever copy-paste
        let mut nodes= self.nodes.borrow_mut();
        let id_ = nodes.len();
        let newlen = id_ + 1;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        // /copy-paste
        nodes.push(Some(Node::Preserialized(val.into_arc())));
        // copy-paste
        Ok(AId::new(self.regionid, id_ as u32))
    }
        

    // Allocate a range of AId:s. We never need to allocate ranges of
    // Node or attribute values, those are only pushed one by one--we
    // need alloc for AVec only and those only store AId:s.
    fn alloc(
        &self,
        n: u32,
        copy_range: Option<(u32, u32)>
    ) -> Result<u32> {
        let mut v = self.ids.borrow_mut();
        let id = v.len();
        let newlen = id + n as usize;
        if newlen > self.max_id as usize {
            bail!("Allocator: out of memory")
        }
        if let Some((start, end)) = copy_range {
            let oldn = end - start;
            assert!(oldn < n);
            v.extend_from_within(start as usize..end as usize);
        }
        v.resize(newlen, u32::MAX); // XX weak marker for invalid id
        Ok(id as u32)
    }

    // // only for ids
    // fn memmove<T>(&self, from: AId<&'a T>, to: AId<&'a T>, len: u32) {
    // }

    pub fn staticstr(
        &self,
        s: &'static str
    ) -> Result<AId<Node>>
    {
        self.new_string(KString::from_static(s))
    }

    pub fn str(
        &self,
        s: &str
    ) -> Result<AId<Node>>
    {
        self.new_string(KString::from_ref(s))
    }

    // (XX hmm, has issue with not offering KString &'static str
    // optimization, right? This is only a small issue, though. Yes,
    // use `str` method for that. AH, staticstr, rename it.)
    pub fn text<T>(
        &self,
        s: T
    ) -> Result<AId<Node>>
    where KString: MyFrom<T>
    {
        self.new_string(KString::myfrom(s))
    }

    // XX remove now that there's text()?
    pub fn string(
        &self,
        s: String
    ) -> Result<AId<Node>>
    {
        self.new_string(KString::from(s))
    }

    // crazy with so many variants?, use a conversion trait?
    pub fn opt_string(
        &self,
        s: Option<String>
    ) -> Result<AId<Node>>
    {
        match s {
            Some(s) => self.new_string(KString::from(s)),
            None => self.empty_node(),
        }
    }

    // XX remove now that there's text()?
    pub fn kstring(
        &self,
        s: KString
    ) -> Result<AId<Node>>
    {
        self.new_string(s)
    }


    /// Create an element from normal slices or arrays, for nice to use
    /// syntax.
    pub fn element(
        &self,
        meta: &'static ElementMeta,
        attr: impl ToASlice<(KString, KString)>,
        body: impl ToASlice<Node>
    ) -> Result<AId<Node>>
    {
        self.new_element(meta,
                         attr.to_aslice(self)?,
                         body.to_aslice(self)?)
    }

    /// A text node with just a non-breaking space.
    pub fn nbsp(&self) -> Result<AId<Node>>
    {
        // Cache and re-issue the same node?
        self.str("\u{00A0}")
    }

    pub fn empty_slice<T>(&self) -> ASlice<T> {
        ASlice {
            t: PhantomData,
            regionid: self.regionid,
            len: 0,
            start: 0,
        }
    }
}

include!("../includes/ahtml_elements_include.rs");


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RegionId {
    allocator_id: U24, // constant
    generation: u8, // mutated
}

#[derive(Debug)]
pub struct AId<T> {
    t: PhantomData<fn() -> T>,
    regionid: RegionId,
    id: u32,
}
impl<T> AId<T> {
    fn new(regionid: RegionId, id: u32) -> AId<T> {
        AId { t: PhantomData, regionid, id }
    }
}

// derive is broken when using PhantomData, so do it manually:
impl<T> Clone for AId<T> {
    fn clone(&self) -> Self {
        Self { t: PhantomData, regionid: self.regionid, id: self.id }
    }
}
impl<T> Copy for AId<T> {}


// AVec lives *outside* an allocator
pub struct AVec<'a, T: AllocatorType> {
    t: PhantomData<T>,
    allocator: &'a HtmlAllocator,
    len: u32,
    cap: u32,
    start: u32, // bare Id for ids
}

impl<'a, T: AllocatorType> AVec<'a, T> {
    // But actually keep private, only instantiate via Allocator::new_vec ?
    pub fn new(allocator: &'a HtmlAllocator) -> AVec<'a, T> {
        AVec {
            t: PhantomData,
            allocator,
            len: 0,
            cap: 0,
            start: 0
        }
    }
    pub fn new_with_capacity(
        allocator: &'a HtmlAllocator,
        capacity: u32
    ) -> Result<AVec<'a, T>> {
        let start = allocator.alloc(
            capacity,
            None)?;
        Ok(AVec {
            t: PhantomData,
            allocator,
            len: 0,
            cap: capacity,
            start
        })
    }
    pub fn len(&self) -> u32 {
        self.len
    }
    pub fn push(&mut self, itemid: AId<T>) -> Result<()> {
        if self.len == self.cap {
            // let oldalloclen = self.allocator.ids.borrow().len();//debug
            let newcap = max(self.cap * 2, 8);
            // We always need space for AIds, not T::allockind()
            let newstart = self.allocator.alloc(
                newcap,
                Some((self.start, self.start + self.len)))?;
            // debug
            // assert!((newstart.0 > self.start.0) ||
            //         // the first allocation is at 0, and 0 is also in start then.
            //         (newstart.0 == 0));
            // let newalloclen = self.allocator.ids.borrow().len();
            // assert_eq!(newalloclen - oldalloclen, newcap as usize);
            // if self.len > 0 {
            //     assert_eq!(self.allocator.get_id::<Node<'a>>(AId::new(self.start.0)).unwrap().0,
            //                self.allocator.get_id::<Node<'a>>(AId::new(newstart.0)).unwrap().0);
            // }
            // /debug
            self.start = newstart;
            self.cap = newcap;
        }
        self.allocator.set_id(self.len + (self.start as usize) as u32, itemid);
        self.len += 1;
        Ok(())
    }
    pub fn as_slice(&self) -> ASlice<T> {
        ASlice {
            t: PhantomData,
            regionid: self.allocator.regionid,
            len: self.len,
            start: self.start
        }
    }
    pub fn reverse(&mut self) {
        let ids = &mut *self.allocator.ids.borrow_mut();
        for i in 0..self.len / 2 {
            // swap(&mut ids[(self.start + i) as usize],
            //      &mut ids[(self.start + self.len - 1 - i) as usize]);
            // nope, can't borrow twice
            // let mut tmp = ids[(self.start + i) as usize];
            // swap(&mut tmp,
            //      &mut ids[(self.start + self.len - 1 - i) as usize]);
            // ids[(self.start + i) as usize] = tmp;
            // Ah!:
            ids.swap((self.start + i) as usize,
                     (self.start + self.len - 1 - i) as usize);
        }
    }

    pub fn push_flat(
        &mut self,
        flat: Flat<T>,
        allocator: &HtmlAllocator
    ) -> Result<()> {
        match flat {
            Flat::None => Ok(()),
            Flat::One(aid) => self.push(aid),
            Flat::Two(a, b) => {
                self.push(a)?;
                self.push(b)?;
                Ok(())
            }
            Flat::Slice(slice) => self.extend_from_slice(&slice, allocator)
        }
    }

    pub fn extend_from_slice(
        &mut self,
        slice: &ASlice<T>,
        allocator: &HtmlAllocator
    ) -> Result<()> {
        for aid in slice.iter_aid(allocator) {
            self.push(aid)?;
        }
        Ok(())
    }
}

// about storage *inside* an allocator, thus no allocator field. XX
// could this be improved?
#[derive(Debug)]
pub struct ASlice<T> {
    t: PhantomData<fn() -> T>,
    regionid: RegionId,
    len: u32,
    start: u32, // id bare to retrieve an AId
}
// again, [derive(Clone)] can't handle it for Clone of T, so do it ourselves:
impl<T> Clone for ASlice<T> {
    fn clone(&self) -> Self {
        Self { t: self.t, regionid: self.regionid, len: self.len, start: self.start }
    }
}
impl<T> Copy for ASlice<T> {}

pub struct ASliceNodeIterator<'a, T> {
    allocator: &'a HtmlAllocator,
    t: PhantomData<T>,
    id: u32,
    id_end: u32,
}
impl<'a, T> Iterator for ASliceNodeIterator<'a, T> {
    type Item = Ref<'a, Node>;
    fn next(&mut self) -> Option<Ref<'a, Node>> {
        if self.id < self.id_end {
            let r = self.allocator.get_id(self.id).expect(
                "slice should always point to allocated storage");
            let v = self.allocator.get_node(r).expect(
                "stored ids should always resolve");
            self.id += 1;
            Some(v)
        } else {
            None
        }
    }
}

// Horrible COPY-PASTE
pub struct ASliceAttIterator<'a, T> {
    allocator: &'a HtmlAllocator,
    t: PhantomData<T>,
    id: u32,
    id_end: u32,
}
impl<'a, T> Iterator for ASliceAttIterator<'a, T> {
    type Item = Ref<'a, (KString, KString)>;
    fn next(&mut self) -> Option<Ref<'a, (KString, KString)>> {
        if self.id < self.id_end {
            let r = self.allocator.get_id(self.id).expect(
                "slice should always point to allocated storage");
            let v = self.allocator.get_att(r).expect(
                "stored ids should always resolve");
            self.id += 1;
            Some(v)
        } else {
            None
        }
    }
}
// /horrible

pub struct ASliceAIdIterator<'a, T> {
    allocator: &'a HtmlAllocator,
    t: PhantomData<T>,
    id: u32,
    id_end: u32,
}
impl<'a, T> Iterator for ASliceAIdIterator<'a, T> {
    type Item = AId<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.id < self.id_end {
            let r = self.allocator.get_id(self.id).expect(
                "slice should always point to allocated storage");
            self.id += 1;
            Some(r)
        } else {
            None
        }
    }
}


impl<'a, T: AllocatorType> ASlice<T> {
    pub fn len(&self) -> u32 { self.len }

    pub fn iter_node(&self, allocator: &'a HtmlAllocator) -> ASliceNodeIterator<'a, T> {
        allocator.assert_regionid(self.regionid);
        ASliceNodeIterator {
            allocator,
            t: PhantomData,
            id: self.start,
            id_end: self.start + self.len
        }
    }
    // Horrible COPY-PASTE
    pub fn iter_att(&self, allocator: &'a HtmlAllocator) -> ASliceAttIterator<'a, T> {
        allocator.assert_regionid(self.regionid);
        ASliceAttIterator {
            allocator,
            t: PhantomData,
            id: self.start,
            id_end: self.start + self.len
        }
    }

    pub fn iter_aid(&self, allocator: &'a HtmlAllocator) -> ASliceAIdIterator<'a, T> {
        ASliceAIdIterator {
            allocator,
            t: PhantomData,
            id: self.start,
            id_end: self.start + self.len
        }
    }

    pub fn try_filter_map<F: Fn(AId<T>) -> Result<Option<AId<T>>>>(
        &self,
        f: F,
        capacity: Option<u32>, // None means self.len() will be used
        allocator: &'a HtmlAllocator
    ) -> Result<AVec<'a, T>> {
        let cap = capacity.unwrap_or_else(|| self.len());
        let mut v = allocator.new_vec_with_capacity(cap)?;
        let end = self.start + self.len;
        for i in self.start..end {
            let id = allocator.get_id(i).expect(
                "slice should always point to allocated storage");
            if let Some(id2) = f(id)? { // XX .with_context ?
                v.push(id2)?; // should never fail if allocated w/ capacity
            }
        }
        Ok(v)
    }

    pub fn try_flat_map<F: Fn(AId<T>) -> Result<Flat<T>>>(
        &self,
        f: F,
        capacity: Option<u32>, // None means self.len() will be used
        allocator: &'a HtmlAllocator
    ) -> Result<AVec<'a, T>> {
        let cap = capacity.unwrap_or_else(|| self.len());
        let mut v = allocator.new_vec_with_capacity(cap)?;
        let end = self.start + self.len;
        for i in self.start..end {
            let id = allocator.get_id(i).expect(
                "slice should always point to allocated storage");
             v.push_flat(f(id)?, allocator)?; // XX .with_context on f's output?
        }
        Ok(v)
    }

    /// Split the slice before the first element for which `f` returns true.
    pub fn split_when<F: Fn(AId<T>) -> bool>(
        &self,
        f: F,
        allocator: &'a HtmlAllocator
    ) -> Option<(ASlice<T>, ASlice<T>)> {
        let end = self.start + self.len;
        for place in self.start..end {
            let id = allocator.get_id(place).expect(
                "slice should always point to allocated storage");
            if f(id) {
                return Some((
                    ASlice {
                        t: PhantomData,
                        regionid: self.regionid,
                        start: self.start,
                        len: place - self.start
                    },
                    ASlice {
                        t: PhantomData,
                        regionid: self.regionid,
                        start: place,
                        len: end - place
                    }
                ))
            }
        }
        None
    }

    /// Split the slice at position `i`, if that is within the slice.
    pub fn split_at(
        &self,
        i: u32,
    ) -> Option<(ASlice<T>, ASlice<T>)> {
        if i <= self.len {
            Some((
                ASlice {
                    t: PhantomData,
                    regionid: self.regionid,
                    start: self.start,
                    len: i
                },
                ASlice {
                    t: PhantomData,
                    regionid: self.regionid,
                    start: self.start + i,
                    len: self.len - i
                }
            ))
        } else {
            None
        }
    }

    /// The first element and the rest, unless the slice is empty.
    pub fn first_and_rest(
        &self,
        allocator: &'a HtmlAllocator
    ) -> Option<(AId<T>, ASlice<T>)> {
        if self.len >= 1 {
            let id = allocator.get_id(self.start).expect(
                "slice should always point to allocated storage");
            Some((
                id,
                ASlice {
                    t: PhantomData,
                    regionid: self.regionid,
                    start: self.start + 1,
                    len: self.len - 1
                }
            ))
        } else {
            None
        }
    }

    pub fn get(&self, i: u32, allocator: &'a HtmlAllocator) -> Option<AId<T>> {
        if i < self.len {
            let id = self.start + i;
            allocator.get_id(id)
        } else {
            None
        }
    }
}

fn unwrap_node(
    node: &Node,
    meta: &ElementMeta,
    strict: bool
) -> Option<ASlice<Node>> {
    match node {
        Node::Element(e) =>
            if (! strict) || e.attr.len == 0 {
                Some(e.body.clone())
            } else {
                None
            },
        Node::String(_) => None,
        Node::Preserialized(p) =>
            if p.meta == meta {
                warn!("can't unwrap_element of preserialized node");
                None
            } else {
                None
            },
        Node::None => None,
    }
}

impl<'a> ASlice<Node> {
    /// If this slice contains only one element of kind `meta` (and
    /// that element has no attributes, if `strict` is true), returns
    /// that element's body slice.
    pub fn unwrap_element_opt(&self,
                              meta: &ElementMeta,
                              strict: bool,
                              allocator: &'a HtmlAllocator) -> Option<ASlice<Node>> {
        if self.len == 1 {
            let nodeid = self.get(0, allocator).expect("exists because len == 1");
            let node = allocator.get_node(nodeid).expect(
                "exists because checked when entered into slice" // was it?
            );
            unwrap_node(&*node, meta, strict)
        } else {
            None
        }
    }

    /// If this slice contains only one element of kind `meta` (and
    /// that element has no attributes, if `strict` is true), returns
    /// that element's body slice, otherwise itself.
    pub fn unwrap_element(&self,
                          meta: &ElementMeta,
                          strict: bool,
                          allocator: &'a HtmlAllocator) -> ASlice<Node> {
        self.unwrap_element_opt(meta, strict, allocator).unwrap_or_else(
            || self.clone())
    }

    /// Unwrap even if there are multiple elements, unwrap all of
    /// those matching `meta`.
    pub fn unwrap_elements(&self,
                           meta: &ElementMeta,
                           strict: bool,
                           allocator: &'a HtmlAllocator) -> Result<ASlice<Node>> {
        self.unwrap_element_opt(meta, strict, allocator).map_or_else(
            || -> Result<ASlice<Node>> {
                let mut v = allocator.new_vec();
                for id in self.iter_aid(allocator) {
                    let node = allocator.get_node(id).expect(
                        // as long as region id is correct? todo: details again?
                        "nodes from slices should always be found?");
                    if let Some(subslice) = unwrap_node(&*node, meta, strict) {
                        for id in subslice.iter_aid(allocator) {
                            v.push(id)?;
                        }
                    } else {
                        v.push(id)?;
                    }
                }
                Ok(v.as_slice())
            },
            Ok)
    }
}

impl<T: AllocatorType> Print for ASlice<T> {
    fn print_html(&self, out: &mut impl Write, allocator: &HtmlAllocator)
                  -> Result<()> {
        for node in self.iter_node(allocator) {
            node.print_html(out, allocator)?;
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
    fn print_html(&self, out: &mut impl Write, allocator: &HtmlAllocator)
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


// lives *inside* an allocator only, thus no allocator field.
#[derive(Debug)]
pub enum Node {
    Element(Element),
    String(KString),
    Preserialized(Arc<SerHtmlFrag>),
    None,
}

impl Node {
    pub fn as_element(&self) -> Option<&Element> {
        match self {
            Node::Element(e) => Some(e),
            Node::String(_) => None,
            Node::Preserialized(_) => None,
            Node::None => None,
        }
    }
    pub fn try_element(&self) -> Result<&Element> {
        match self {
            Node::Element(e) => Ok(e),
            Node::String(_) =>
                bail!("not a Node::String, but Node::Preserialized"),
            Node::Preserialized(_) =>
                bail!("not an Node::Element, but Node::Preserialized"),
            Node::None => 
                bail!("not an Node::Element, but Node::None"),
        }
    }
}

impl Print for Node {
    fn print_html(&self, out: &mut impl Write, allocator: &HtmlAllocator) -> Result<()>
    {
        Ok(match self {
            Node::Element(e) => e.print_html(out, allocator)?,
            Node::String(s) => out.write_all(&allocator.html_escape(s.as_bytes()))?,
            Node::Preserialized(ser) =>
                out.write_all(ser.kstring.as_bytes())?,
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


// lives *inside* an allocator only via Node, thus no allocator field.
/// Invalid `Element`s can definitely be built (non-allowed child
/// elements), but still has public fields since it will be plucked
/// apart and verified in `allocate_element` before being stored. And
/// there's no mut access to the store.
#[derive(Debug, Clone)]
pub struct Element {
    pub meta: &'static ElementMeta,
    pub attr: ASlice<(KString, KString)>,
    pub body: ASlice<Node>,
}

impl Element {
    pub fn meta(&self) -> &'static ElementMeta { self.meta }
    pub fn attr(&self) -> &ASlice<(KString, KString)> { &self.attr }
    pub fn body(&self) -> &ASlice<Node> { &self.body }

    pub fn try_filter_map_body<'a, T: AllocatorType>(
        &self,
        f: impl Fn(AId<Node>) -> Result<Option<AId<Node>>>,
        allocator: &'a HtmlAllocator
    ) -> Result<Element> {
        let body2 = self.body.try_filter_map(f, None, allocator)?;
        Ok(Element {
            meta: self.meta,
            attr: self.attr.clone(),
            body: body2.as_slice()
        })
    }

    pub fn try_flat_map_body<'a, T: AllocatorType>(
        &self,
        f: impl Fn(AId<Node>) -> Result<Flat<Node>>,
        allocator: &'a HtmlAllocator
    ) -> Result<Element> {
        let body2 = self.body.try_flat_map(f, None, allocator)?;
        Ok(Element {
            meta: self.meta,
            attr: self.attr.clone(),
            body: body2.as_slice()
        })
    }
}

impl Print for Element {
    fn print_html(&self, out: &mut impl Write, allocator: &HtmlAllocator)
             -> Result<()>
    {
        let meta = self.meta;
        // meta.has_global_attributes XX ? only for verification?
        out.write_all(b"<")?;
        out.write_all(meta.tag_name.as_bytes())?;
        for att in self.attr.iter_att(allocator) {
            out.write_all(b" ")?;
            att.print_html(out, allocator)?;
        }
        out.write_all(b">")?;
        self.body.print_html(out, allocator)?;
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


/// Serialized HTML fragment string. Can be included in
/// Node:s. Contains the metainformation about the outermost element
/// in the serialized fragment for dynamic DOM checking.
#[derive(Debug)]
pub struct SerHtmlFrag {
    meta: &'static ElementMeta,
    kstring: KString
}



#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    #[test]
    fn t_siz() {
        assert_eq!(size_of::<RegionId>(), 4);
        assert_eq!(size_of::<AId<Node>>(), 8);
    }
}
