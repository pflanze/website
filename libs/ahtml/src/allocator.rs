use std::{sync::{Mutex, atomic::AtomicBool, Arc},
          cell::RefCell,
          collections::HashSet,
          marker::PhantomData,
          cmp::max};

use anyhow::{bail, Result, anyhow};
use ahtml_html::meta::{MetaDb, ElementMeta};
use backtrace::Backtrace;
use chj_util::{u24::{U24, U24MAX}, partialbacktrace::PartialBacktrace, warn};
use kstring::KString;
use lazy_static::lazy_static;

use crate::{myfrom::MyFrom, arc_util::IntoArc, more_vec::MoreVec, stillvec::StillVec};

// once again
fn all_whitespace(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_whitespace())
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


pub struct AllocatorPool {
    max_id: u32, // See Allocator
    metadb: Option<&'static MetaDb>, // See Allocator
    allocators: Mutex<Vec<HtmlAllocator>>,
}

impl AllocatorPool {
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

pub struct AllocatorGuard<'p> {
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
        let mut a = self._allocator.take().unwrap();
        if a.regionid.generation < 20 {
            a.clear();
            // Insert it back into the pool:
            let mut l = self.pool.allocators.lock().unwrap();
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
    atts: StillVec<Option<(KString, KString)>>,
    // Storage for nodes:
    nodes: StillVec<Option<Node>>,
    // Storage for references to attributes and nodes:
    ids: RefCell<Vec<u32>>, // for attribute or Node, depending on slot
    // Temporary storage for serialisation:
    pub(crate) html_escape_tmp: RefCell<Vec<u8>>,
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

pub trait ToASlice<T> {
    fn to_aslice(self, allocator: &HtmlAllocator) -> Result<ASlice<T>>;
}

pub static AHTML_TRACE: AtomicBool = AtomicBool::new(false);

impl HtmlAllocator {
    pub fn new_with_metadb(max_allocations: u32, metadb: Option<&'static MetaDb>) -> Self {
        let max_allocations = max_allocations as usize;
        let half_max_alloc = max_allocations / 2;
        HtmlAllocator {
            regionid: RegionId {
                allocator_id: next_allocator_id(),
                generation: 0,
            },
            // Assume that attributes are relatively rare, even
            // half_max_alloc seems overly many, well.
            atts: StillVec::with_capacity(half_max_alloc),
            // Even though ids <= nodes + atts, we don't know how the
            // distribution between nodes and atts will be, so have to
            // allocate nodes with (close to) the max, too.
            nodes: StillVec::with_capacity(max_allocations),
            ids: RefCell::new(Vec::with_capacity(max_allocations)),
            metadb,
            html_escape_tmp: RefCell::new(Vec::new()),
        }
    }

    pub fn clear(&mut self) {
        self.atts.exclusive_clear();
        self.nodes.exclusive_clear();
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

    pub fn get_node<'a>(&'a self, id: AId<Node>) -> Option<&'a Node> {
        if let Some(v) = self.nodes.get(self.id_to_index(id)) {
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
    }

    // COPY-PASTE of above
    pub fn get_att<'a>(&'a self, id: AId<(KString, KString)>)
                   -> Option<&'a (KString, KString)>
    {
        if let Some(v) = self.atts.get(self.id_to_index(id)) {
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
    }

    // For within ids. To get the id, to be of type T.
    pub fn get_id<T: AllocatorType>(&self, id_bare: u32) -> Option<AId<T>> {
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
        
        let id_ = self.nodes.len();
        self.nodes.push_within_capacity_(Some(Node::Element(Element {
            meta,
            attr,
            body
        }))).map_err(|_| anyhow!("HtmlAllocator: out of memory"))?;
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
        let id_ = self.nodes.len();
        self.nodes.push_within_capacity_(Some(Node::String(s)))
            .map_err(|_| anyhow!("HtmlAllocator: out of memory"))?;
        Ok(AId::new(self.regionid, id_ as u32))
    }
    pub fn empty_node(&self) -> Result<AId<Node>> {
        // much COPY-PASTE always
        let id_ = self.nodes.len();
        self.nodes.push_within_capacity_(Some(Node::None))
            .map_err(|_| anyhow!("HtmlAllocator: out of memory"))?;
        Ok(AId::new(self.regionid, id_ as u32))
    }

    pub fn new_attribute(
        &self,
        att: (KString, KString)
    ) -> Result<AId<(KString, KString)>>
    {
        let id_ = self.atts.len();
        self.atts.push_within_capacity_(Some(att))
            .map_err(|_| anyhow!("HtmlAllocator: out of memory"))?;
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
        let id_ = self.nodes.len();
        // /copy-paste
        self.nodes.push_within_capacity_(Some(Node::Preserialized(val.into_arc())))
            .map_err(|_| anyhow!("HtmlAllocator: out of memory"))?;
        // copy-paste
        Ok(AId::new(self.regionid, id_ as u32))
    }

    // Allocate a range of `AId`s. We never need to allocate ranges of
    // Node or attribute values, those are only pushed one by one--we
    // need alloc for `AVec` only and those only store `AId`s.
    // Giving a `copy_range` essentially makes this a "realloc".
    fn alloc(
        &self,
        n: u32,
        copy_range: Option<(u32, u32)>
    ) -> Result<u32> {
        let mut v = self.ids.borrow_mut();
        let id = v.len();
        let newlen = id + n as usize;
        if newlen > v.capacity() {
            bail!("HtmlAllocator: out of memory")
        }
       
        if let Some((start, end)) = copy_range {
            let oldn = end - start;
            assert!(oldn < n);
            v.extend_from_within_within_capacity(start as usize..end as usize)
                .expect("can't happen since we checked newlen above");
        }

        // And additionally / in any case extend with the new space. Use `u32::MAX`
        // as a weak marker for invalid id
        v.resize(newlen, u32::MAX);
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

    // /// Create a transparent pseudo element with the given body; that
    // /// body is flattened into the element's body where it is placed.
    // pub fn flat(
    //     &self,
    //     body: impl ToASlice<Node>
    // ) -> Result<AId<Node>>
    // {
    //     body.to_aslice(self)
    // }

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

impl<T: AllocatorType> AId<T> {
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
/// A vector that allocates its storage from a `HtmlAllocator`. When
/// finished, convert to `ASlice` via `as_slice()`.
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

    #[inline(always)]
    pub fn allocator(&self) -> &'a HtmlAllocator {
        self.allocator
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

    pub fn append<S: Into<ASlice<T>>>(&mut self, elements: S) -> Result<()> {
        let aslice: ASlice<T> = elements.into();
        for aid in aslice.iter_aid(self.allocator) {
            self.push(aid)?;
        }
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
            ids.swap((self.start + i) as usize,
                     (self.start + self.len - 1 - i) as usize);
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
/// A slice of stored `AId<T>`s inside a `HtmlAllocator`.
#[derive(Debug)]
pub struct ASlice<T> {
    t: PhantomData<fn() -> T>,
    regionid: RegionId,
    pub(crate) len: u32,
    pub(crate) start: u32, // id bare to retrieve an AId
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
    type Item = &'a Node;
    fn next(&mut self) -> Option<&'a Node> {
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
    type Item = &'a (KString, KString);
    fn next(&mut self) -> Option<&'a (KString, KString)> {
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
impl<'a, T: AllocatorType> Iterator for ASliceAIdIterator<'a, T> {
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

impl<'a, T: AllocatorType> IntoIterator for AVec<'a, T> {
    type Item = AId<T>;

    type IntoIter = ASliceAIdIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        ASliceAIdIterator {
            allocator: self.allocator,
            t: PhantomData,
            id: self.start,
            id_end: self.start + self.len,
        }
    }
}

// stupid copy-paste with 1 character added:
impl<'a, T: AllocatorType> IntoIterator for &AVec<'a, T> {
    type Item = AId<T>;

    type IntoIter = ASliceAIdIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        ASliceAIdIterator {
            allocator: self.allocator,
            t: PhantomData,
            id: self.start,
            id_end: self.start + self.len,
        }
    }
}

// Note: can't implement IntoIterator for ASlice, because ASlice does
// not have a reference to HtmlAllocator and IntoIterator does not
// allow to take one. See `iter_aid` method on ASlice instead.


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


/// Serialized HTML fragment string. Can be included in
/// Node:s. Contains the metainformation about the outermost element
/// in the serialized fragment for dynamic DOM checking.
#[derive(Debug)]
pub struct SerHtmlFrag {
    pub(crate) meta: &'static ElementMeta,
    pub(crate) kstring: KString
}

impl SerHtmlFrag {
    #[inline(always)]
    pub fn meta(&self) -> &'static ElementMeta {
        self.meta
    }
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        &self.kstring
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
}


#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    #[test]
    fn t_system_at_least_32bits() {
        // We use `n as usize` etc. everywhere, where n is u32. Make
        // sure this is OK.
        let n: u32 = u32::MAX;
        let _x: usize = n.try_into()
            .expect("system has at least 32 bits");
    }

    #[test]
    fn t_siz() {
        assert_eq!(size_of::<RegionId>(), 4);
        assert_eq!(size_of::<AId<Node>>(), 8);
    }
}
