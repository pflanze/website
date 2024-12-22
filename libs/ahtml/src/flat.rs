use anyhow::Result;

use crate::allocator::{AId, ASlice, AllocatorType, ToASlice, HtmlAllocator, AVec, Element, Node};


pub enum Flat<T> {
    None,
    One(AId<T>),
    Two(AId<T>, AId<T>),
    Slice(ASlice<T>)
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

impl<'a, T: AllocatorType> AVec<'a, T> {
    pub fn push_flat(
        &mut self,
        flat: Flat<T>,
    ) -> Result<()> {
        match flat {
            Flat::None => Ok(()),
            Flat::One(aid) => self.push(aid),
            Flat::Two(a, b) => {
                self.push(a)?;
                self.push(b)?;
                Ok(())
            }
            Flat::Slice(slice) => self.extend_from_slice(&slice, self.allocator())
        }
    }
}

impl<'a, T: AllocatorType> ASlice<T> {
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
             v.push_flat(f(id)?)?; // XX .with_context on f's output?
        }
        Ok(v)
    }
}

impl Element {
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

