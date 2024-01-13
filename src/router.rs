use std::{fmt::Debug, borrow::Borrow, slice, iter::Rev};
use anyhow::{Result, bail};
use kstring::KString;

use crate::{myasstr::MyAsStr,
            path::path_segments,
            trie::{Trie, TrieIter, TrieIterReportStyle},
            ppath::PPath};


// Allow single entries as endpoints.
#[derive(Debug)]
pub struct UniqueRouter<T>(Trie<T>);

impl<T> UniqueRouter<T> {
    pub fn new(allow_both: bool) -> UniqueRouter<T> {
        UniqueRouter(Trie::new(allow_both))
    }

    /// Using path *strings*, and chaining.
    pub fn add(&mut self, path: &str, val: T) -> Result<&mut Self>
    where T: Debug
    {
        let pathv: Vec<_> = path_segments(path).collect();
        match self.0.insert(pathv.as_slice(), val)? {
            Some(old) => bail!("already contained an entry for {:?}: {:?}",
                               path, old),
            None => Ok(self)
        }
    }

    pub fn get<P: Eq + MyAsStr + Debug + Clone>(
        &self,
        path: &PPath<P>
    ) -> Option<(&T, PPath<P>)>
    where KString: Borrow<str>
    {
        let (val, path1) = self.0.get(path.segments())?;
        Some((val, PPath::new(false, path.ends_with_slash(), path1.into())))
    }

    pub fn get_trie<P: Eq + MyAsStr + Debug + Clone>(
        &self,
        path: &PPath<P>
    ) -> Option<&Trie<T>>
    where KString: Borrow<str>
    {
        self.0.get_leaf(path.segments())
    }
    
    // Convenience function for path *strings*
    // pub fn get(&self, path: &str) -> bool {
    //     self.get_path(path_seqments(path).as_slice())
    // }
    // oh, can't do this bc can't return slice to local Vec in return.

    pub fn trie(&self) -> &Trie<T> {
        &self.0
    }
    pub fn trie_mut(&mut self) -> &mut Trie<T> {
        &mut self.0
    }

    // Can't do this since there's no sub-&Self, and can't make a
    // UniqueRouter around a sub-Trie because it wants to own it.
    // pub fn get_router(&self) -> Option<&Self> {}
    // So, instead provide a combined get and iter method:
    /// Look up path, if there's a Trie leaf (regardless of whether
    /// it holds an endpoint), return an iterator on it.
    pub fn get_iter<'trie, 'p, P: Eq + MyAsStr>(
        &'trie self,
        path: &'p [P],
        direction_backwards: bool,
        report_style: TrieIterReportStyle
    ) -> Option<UniqueRouterIter<'trie, T>>
    where KString: Borrow<str>
    {
        let trie = self.0.get_leaf(path)?;
        Some(UniqueRouterIter {
            trie_iter: trie.iter(direction_backwards, report_style)
        })
    }

    pub fn iter<'trie>(
        &'trie self,
        direction_backwards: bool,
        report_style: TrieIterReportStyle
    ) -> UniqueRouterIter<'trie, T>
    {
        UniqueRouterIter {
            trie_iter: self.0.iter(direction_backwards, report_style)
        }
    }
}

pub struct UniqueRouterIter<'trie, T> {
    trie_iter: TrieIter<'trie, T>,
}

impl<'trie, T> Iterator for UniqueRouterIter<'trie, T> {
    type Item = (Vec<&'trie str>, &'trie T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (path, trie) = self.trie_iter.next()?;
            if let Some(endpoint) = trie.endpoint() {
                return Some((path, endpoint))
            }
        }
    }
}


// Allow multiple entries as endpoints. They shall be tried in
// sequence.
#[derive(Debug)]
pub struct MultiRouter<T>(Trie<Vec<T>>);

impl<T> MultiRouter<T> {
    pub fn new() -> MultiRouter<T> {
        MultiRouter(Trie::new(true))
    }

    /// Using path *strings*, and chaining.
    pub fn add(&mut self, path: &str, val: T) -> &mut Self
    where T: Debug
    {
        let pathv: Vec<_> = path_segments(path).collect();
        let endpoint = self.0.get_endpoint_mut(pathv.as_slice())
            .expect("always succeeds because Trie is constructed with `true`");
        match endpoint {
            Some(v) => v.push(val),
            None => *endpoint = Some(vec![val])
        }
        self
    }

    pub fn get<P: Eq + MyAsStr + Debug + Clone>(
        &self,
        path: &PPath<P>
    ) -> Option<(&Vec<T>, PPath<P>)>
    where KString: Borrow<str>
    {
        let (val, path1) = self.0.get(path.segments())?;
        Some((val, PPath::new(false, path.ends_with_slash(), path1.into())))
    }

    pub fn trie(&self) -> &Trie<Vec<T>> {
        &self.0
    }
    pub fn trie_mut(&mut self) -> &mut Trie<Vec<T>> {
        &mut self.0
    }

    pub fn iter<'trie,
                Dir: Direction<'trie, T, VecIter> + NewDirection<Dir>,
                VecIter: Iterator<Item = &'trie T>,
                >(
        &'trie self,
        report_style: TrieIterReportStyle
    ) -> MultiRouterIter<'trie, T, VecIter, Dir>
    {
        MultiRouterIter {
            direction: Dir::new(),
            node_iter: None,
            trie_iter: self.0.iter(Dir::new().is_backwards(), report_style),
        }
    }
}

pub trait Direction<'trie, T, VecIter> {
    fn iter(&self, vec: &'trie Vec<T>) -> VecIter;
    fn is_backwards(&self) -> bool;
}
pub trait NewDirection<Dir> {
    fn new() -> Dir;
}

pub struct DirectionForward {}
impl NewDirection<DirectionForward> for DirectionForward {
    fn new() -> DirectionForward { DirectionForward{} }
}
pub struct DirectionReverse {}
impl NewDirection<DirectionReverse> for DirectionReverse {
    fn new() -> DirectionReverse { DirectionReverse{} }
}

impl<'trie, T> Direction<'trie, T, slice::Iter<'trie, T>> for DirectionForward {
    fn iter(&self, vec: &'trie Vec<T>) -> slice::Iter<'trie, T> {
        vec.iter()
    }
    fn is_backwards(&self) -> bool {
        false
    }
}
impl<'trie, T> Direction<'trie, T, Rev<slice::Iter<'trie, T>>> for DirectionReverse {
    fn iter(&self, vec: &'trie Vec<T>) -> Rev<slice::Iter<'trie, T>> {
        vec.iter().rev()
    }
    fn is_backwards(&self) -> bool {
        true
    }
}

pub struct MultiRouterIter<'trie,
                           T,
                           VecIter: Iterator<Item = &'trie T>,
                           Dir: Direction<'trie, T, VecIter>> {
    direction: Dir,
    node_iter: Option<(Vec<&'trie str>, VecIter)>,
    trie_iter: TrieIter<'trie, Vec<T>>,
}

impl<'trie,
     T,
     VecIter: Iterator<Item = &'trie T>,
     Dir: Direction<'trie, T, VecIter>>
    Iterator
    for
    MultiRouterIter<'trie, T, VecIter, Dir>
{
    type Item = (Vec<&'trie str>, &'trie T);
    
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((path, vec_iter)) = &mut self.node_iter {
            if let Some(val) = vec_iter.next() {
                // XX cloning for every iteration makes me a bit sick.
                return Some((path.clone(), val))
            } else {
                self.node_iter = None;
            }
        }
        if let Some((path, trie)) = self.trie_iter.next() {
            if let Some(endpoint) = trie.endpoint() {
                self.node_iter = Some((path, self.direction.iter(endpoint)));
            }
            return self.next();
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_add_false() -> Result<()> {
        let mut r = UniqueRouter::new(false);
        r
            .add("/hello/world", 1).unwrap()
            .add("/index.html", 2).unwrap();
        assert_eq!(r.add("/", 2).err().unwrap().to_string(),
                   "there's a longer path continuing after ours ([])");
        Ok(())
    }

    #[test]
    fn t_add_true() -> Result<()> {
        let mut r = UniqueRouter::new(true);
        r
            .add("/hello/world", 1).unwrap()
            .add("/index.html", 2).unwrap()
            .add("/", 2).unwrap()
            ;
        Ok(())
    }

    #[test]
    fn t_iter() -> Result<()> {
        let mut r = MultiRouter::new();
        r
            .add("/hello/world", 1)
            .add("/hello/world", 2)
            .add("/index.html", 3)
            .add("/hello", 4)
            ;
        {
            let v: Vec<_> = r.iter::<DirectionForward, _>(
                TrieIterReportStyle::BeforeRecursing).collect();
            assert_eq!(
                v,
                vec![
                    (vec!["hello"], &4),
                    (vec!["hello", "world"], &1),
                    (vec!["hello", "world"], &2),
                    (vec!["index.html"], &3),
                ]);
        }
        {
            let v: Vec<_> = r.iter::<DirectionForward, _>(
                TrieIterReportStyle::AfterRecursing).collect();
            assert_eq!(
                v,
                vec![
                    (vec!["hello", "world"], &1),
                    (vec!["hello", "world"], &2),
                    (vec!["hello"], &4),
                    (vec!["index.html"], &3),
                ]);
        }
        {
            let v: Vec<_> = r.iter::<DirectionReverse, _>(
                TrieIterReportStyle::BeforeRecursing).collect();
            assert_eq!(
                v,
                vec![
                    (vec!["index.html"], &3),
                    (vec!["hello"], &4),
                    (vec!["hello", "world"], &2),
                    (vec!["hello", "world"], &1),
                ]);
        }
        Ok(())
    }


}
