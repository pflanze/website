//! A trie where each level uses a `BTreeMap` for branching. The goal
//! is not performance but the ability to list all the sub-tries at
//! every level.

use std::{collections::{BTreeMap, btree_map}, fmt::{Debug, Display}, borrow::Borrow};
use ahtml::myfrom::MyFrom;
use anyhow::{Result, bail};
use kstring::KString;

use chj_util::{nodt as dt, slice::first_and_rest, myasstr::MyAsStr};
use ahtml_from_markdown::util::{debug_stringlikes, btreemap_try_insert, btreemap_get_mut};

#[allow(dead_code)]
fn debug_path<P: Eq + MyAsStr>(
    path: &[P]
) -> Vec<&str> {
    path.iter().map(|s| s.my_as_str()).collect()
}
    


// FUTURE: Make Trie independent of string assumptions, by moving to
// hashbrown, eliminating Borrow, also eliminate `anyhow`.

#[derive(Debug)]
pub struct Trie<T> {
    allow_both: bool, // looked at for insertions only, not lookups
    branching: Option<BTreeMap<KString, Trie<T>>>,
    endpoint: Option<T>,
}
impl<T> Trie<T> {
    /// If `allow_both` is true, the trie will allow overlays of
    /// paths and endpoints, i.e. paths continuing from where an
    /// endpoint lies; lookups will give more specific path matches
    /// priority. If false, will report an error if an endpoint is to
    /// be added in the middle of an existing path, as well as when a
    /// path is added that overshoots an existing endpoint.
    pub fn new(allow_both: bool) -> Trie<T> {
        Trie {
            allow_both,
            branching: None,
            endpoint: None
        }
    }

    /// Resolves the path as far as possible and returns the last leaf
    /// and the remainder of the path.
    pub fn get_leaf_rest<'p, P: Eq + MyAsStr>(
        &self,
        path: &'p [P]
    ) -> (&Self, &'p [P])
    where KString: Borrow<str>
    {
        if let Some((fst, rst)) = first_and_rest(path) {
            if let Some(branching) = &self.branching {
                if let Some(trie) = branching.get(fst.my_as_str()) {
                    return trie.get_leaf_rest(rst)
                }
            }
        }
        (self, path)        
    }

    pub fn get_leaf<'p, P: Eq + MyAsStr>(
        &self,
        path: &'p [P]
    ) -> Option<&Self>
    where KString: Borrow<str>
    {
        let (leaf, path2) = self.get_leaf_rest(path);
        if path2.is_empty() {
            Some(leaf)
        } else {
            None
        }
    }
    
    /// Returns the value and the remainder of the path on a match.
    pub fn get<'p, P: Eq + MyAsStr>(
        &self,
        path: &'p [P]
    ) -> Option<(&T, &'p [P])>
    where KString: Borrow<str>
    {
        dt!("trie get", debug_path(path));
        // Try to eagerly match as much as possible
        if let Some((fst, rst)) = first_and_rest(path) {
            if let Some(branching) = &self.branching {
                if let Some(trie) = branching.get(fst.my_as_str()) {
                    if let Some(match_) = trie.get(rst) {
                        dt!("trie get match", debug_path(match_.1));
                        return Some(match_)
                    }
                }
            }
        }
        // If the path is empty, we report the result, same as when it
        // isn't but failed to lead to a more eager match.
        if let Some(endpoint) = &self.endpoint {
            dt!("trie get fallback", debug_path(path));
            return Some((endpoint, path))
        }
        None
    }

    /// Returns a reference to the leaf node, extending the tree if
    /// necessary (copying the value of `allow_both` from the one in
    /// the node it is extended from). Returns an error if encountering
    /// an EndPoint on the way and `allow_both` in that node is false.
    pub fn get_leaf_mut<'trie, 'p, P: Eq + MyAsStr>(
        &'trie mut self,
        path: &'p [P]
    ) -> Result<&'trie mut Trie<T>,
                (&'trie mut Trie<T>, &'p [P])>
    where KString: Borrow<str> + MyFrom<&'p P>
    {
        let cont =
            |slf: &'trie mut Trie<T>, (fst, rst) | -> Result<&'trie mut Trie<T>,
                                                             (&'trie mut Trie<T>, &'p [P])>
        {
            let branching = slf.branching.as_mut().unwrap();
            match btreemap_get_mut(branching, path[0].my_as_str()) {
                Ok(trie) => {
                    trie.get_leaf_mut(rst)
                }
                Err(branching) => {
                    // Not using .expect() here because that would require Debug on T.
                    match btreemap_try_insert(branching,
                                              KString::myfrom(fst),
                                              Trie::new(slf.allow_both)) {
                        Ok(trie) => trie.get_leaf_mut(rst),
                        Err(_) => panic!("we just looked and the spot was empty")
                    }
                }
            }
        };

        if let Some(fst_rst) = first_and_rest(path) {
            if self.branching.is_some() {
                cont(self, fst_rst)
            } else {
                // no such node yet; extend, but only if allowed
                if self.allow_both || self.endpoint.is_none() {
                    let b = BTreeMap::new();
                    self.branching = Some(b);
                    cont(self, fst_rst)
                } else {
                    Err((self, path))
                }
            }
        } else {
            Ok(self)
        }
    }

    pub fn _get_endpoint_mut<'trie, 'p, P: Eq + MyAsStr>(
        &'trie mut self,
        path: &'p [P]
    ) -> Result<&'trie mut Option<T>,
                (&'trie mut Trie<T>, &'p [P])>
    where KString: Borrow<str> + MyFrom<&'p P>
    {
        let leaf = self.get_leaf_mut(path)?;
        Ok(&mut leaf.endpoint)
    }

    pub fn get_endpoint_mut<'trie, 'p, P: Eq + MyAsStr + Display>(
        &'trie mut self,
        path: &'p [P]
    ) -> Result<&'trie mut Option<T>>
    where KString: Borrow<str> + MyFrom<&'p P>
    {
        match self.get_leaf_mut(path) {
            Err((_r, rest)) => {
                let i = path.len() - rest.len();
                let p0 = &path[0..i];
                bail!("there's an end point between the path segments {:?} and {:?}",
                      debug_stringlikes(p0),
                      debug_stringlikes(rest))
            }
            Ok(trie) =>
                if trie.allow_both || trie.branching.is_none() {
                    Ok(&mut trie.endpoint)
                } else {
                    bail!("there's a longer path continuing after ours ({:?})",
                          debug_stringlikes(path))
                }
        }
    }

    /// Get the endpoint of the local node, if allowed, otherwise give
    /// an error. If allowed, still returns an Option since may
    /// currently be unfilled.
    // (Sigh, duplicating error logic. todo: move out. Can't just be
    // called whole sale because current vs extend is not same, hmm?
    // Well, the message is already different.)
    pub fn endpoint_mut(&mut self) -> Result<&mut Option<T>>
    {
        if self.allow_both || self.branching.is_none() {
            Ok(&mut self.endpoint)
        } else {
            bail!("there's a longer path continuing after this node")
        }
    }

    /// Get the endpoint if present. (There is no distinction between
    /// being allowed and just not having an endpoint.)
    pub fn endpoint(&self) -> Option<&T>
    {
        self.endpoint.as_ref()
    }
    
    /// Returns the old value if path was already contained. Returns an
    /// Error if `path` is crossing an existing EndPoint, or reaching
    /// partially into an existing path.
    pub fn insert<'trie, 'p, P: 'p + Eq + MyAsStr + Display>(
        &'trie mut self,
        path: &'p [P],
        val: T
    ) -> Result<Option<T>>
    where KString: Borrow<str> + MyFrom<&'p P>
    {
        let endpoint = self.get_endpoint_mut(path)?;
        let oldendpoint = endpoint.take();
        *endpoint = Some(val);
        Ok(oldendpoint)
    }

    /// Iterater over the Trie returning Trie nodes. The Iterator is
    /// not double-ended, meaning .rev() cannot be called on it; as a
    /// workaround, `direction_backwards` can be set to `true`
    /// here. Also, you get to choose whether the Trie nodes should be
    /// shown before or after showing their children (or not at all,
    /// which doesn't work yet XX).
    pub fn iter<'trie>(
        &'trie self,
        direction_backwards: bool,
        report_style: TrieIterReportStyle
    ) -> TrieIter<'trie, T> {
        let mut continuation = Vec::new();
        // XX this TrieIterContFrame should be a method; see copy below
        // another "TrieIterContFrame {"
        let branches = self.branching.as_ref().and_then(
            |m| Some(m.iter()));
        continuation.push(TrieIterContFrame {
            state: TrieIterContState::Before,
            node: self,
            path_segment_leading_here: None,
            branches,
        });
        TrieIter {
            direction_backwards,
            report_style,
            continuation
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrieIterReportStyle {
    BeforeRecursing,
    AfterRecursing,
    NotAtAll
}

#[derive(Debug, Clone, Copy)]
enum TrieIterContState {
    Before,
    Iterate,
}

struct TrieIterContFrame<'trie, T>{
    state: TrieIterContState,
    node: &'trie Trie<T>,
    path_segment_leading_here: Option<&'trie str>,
    branches: Option<btree_map::Iter<'trie, KString, Trie<T>>>, // only if branching
}

pub struct TrieIter<'trie, T>
{
    direction_backwards: bool, // for iter over branches; no effect on report_style
    report_style: TrieIterReportStyle,
    continuation: Vec<TrieIterContFrame<'trie, T>>,
}
impl<'trie, T> TrieIter<'trie, T> {
    pub fn current_path(&self) -> Vec<&'trie str> {
        self.continuation.iter().filter_map(
            |c| c.path_segment_leading_here).collect()
    }
}

macro_rules! ondebug {
    ($($t:tt)*) => {
        // $($t)*;
    }
}

macro_rules! debug {
    ($($t:tt)*) => {
        ondebug!(eprintln!($($t)*))
    }
}

impl<'trie, T> Iterator for TrieIter<'trie, T>
{
    type Item = (Vec<&'trie str>, &'trie Trie<T>);

    fn next(&mut self) -> Option<Self::Item>
    {
        ondebug!(let current_path_debug = self.current_path());
        let cont = self.continuation.last_mut()?;
        debug!("next {:?} {:?}", current_path_debug, cont.state);
        match cont.state {
            TrieIterContState::Before => {
                cont.state = TrieIterContState::Iterate;
                if self.report_style == TrieIterReportStyle::BeforeRecursing {
                    debug!("next {:?} {:?}: report node",
                           current_path_debug, cont.state);
                    let node = cont.node;
                    Some((self.current_path(), node))
                } else {
                    // go to iteration, easiest via:
                    debug!("next {:?} {:?}: skip to iteration",
                           current_path_debug, cont.state);
                    self.next()
                }
            }
            TrieIterContState::Iterate => {
                if let Some(branches) = &mut cont.branches {
                    if let Some((pathsegment, trie)) =
                        if self.direction_backwards {
                            branches.next_back()
                        } else {
                            branches.next()
                        }
                    {
                        debug!("next {:?} {:?}: recurse",
                               current_path_debug, cont.state);
                        let branches = trie.branching.as_ref().and_then(
                            |m| Some(m.iter()));
                        self.continuation.push(TrieIterContFrame {
                            state: TrieIterContState::Before,
                            node: trie,
                            path_segment_leading_here: Some(pathsegment.as_str()),
                            branches,
                        });
                        return self.next()
                    }
                }
                let node = cont.node;
                ondebug!(let state = cont.state);
                // drop(cont);
                let current_path = self.current_path();
                let _ = self.continuation.pop().expect("saw it above");
                if self.report_style == TrieIterReportStyle::AfterRecursing {
                    debug!("next {:?} {:?}: report node + return",
                           current_path_debug, state);
                    Some((current_path, node))
                } else {
                    debug!("next {:?} {:?}: return", current_path_debug, state);
                    self.next()
                }
            }
        }
    }
}

// This seems too painful. TrieIter does support reverse direction
// directly.
// impl<T> DoubleEndedIterator for &Trie<T> {
//     fn next_back(&mut self) -> Option<Self::Item> {
//     }
// }



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_false() -> Result<()> {
        let mut r = Trie::new(false);
        r.insert(&["foo", "bar"], 10).unwrap();
        r.insert(&["foo", "baz"], 11).unwrap();
        r.insert(&["bum", "baz"], 12).unwrap();
        match r.insert(&["foo", "baz", "bam"], 12) {
            Ok(_) => panic!("wrong"),
            Err(e) => assert_eq!(
                e.to_string(),
                "there's an end point between the path segments \
                 [\"foo\", \"baz\"] and [\"bam\"]"),
        }
        match r.insert(&["foo"], 13) {
            Ok(_) => panic!("wrong"),
            Err(e) => assert_eq!(
                e.to_string(),
                "there's a longer path continuing after ours ([\"foo\"])"),
        }
        assert_eq!(r.get(&["Foo"]), None);
        assert_eq!(r.get(&["foo"]), None);
        assert_eq!(r.get::<&str>(&[]), None);
        assert_eq!(r.get(&["foo", "bar"]), Some((&10, [].as_slice())));
        assert_eq!(r.get(&["foo", "bar", "baz"]), Some((&10, ["baz"].as_slice())));
        Ok(())
    }

    // adapted copy-paste
    #[test]
    fn t_true() -> Result<()> {
        let mut r = Trie::new(true);
        r.insert(&["foo", "bar"], 10).unwrap();
        r.insert(&["foo", "baz"], 11).unwrap();
        r.insert(&["bum", "baz"], 12).unwrap();
        match r.insert(&["foo", "baz", "bam"], 12) {
            Ok(old) => assert_eq!(old, None),
            Err(_e) => panic!("wrong"),
        }
        match r.insert(&["foo"], 13) {
            Ok(old) => assert_eq!(old, None),
            Err(_e) => panic!("wrong"),
        }
        assert_eq!(r.get(&["Foo"]), None);
        assert_eq!(r.get(&["foo"]), Some((&13, [].as_slice())));
        assert_eq!(r.get::<&str>(&[]), None);
        assert_eq!(r.get(&["foo", "bar"]), Some((&10, [].as_slice())));
        assert_eq!(r.get(&["foo", "bar", "baz"]), Some((&10, ["baz"].as_slice())));
        Ok(())
    }
    
    #[test]
    fn t_iter() {
        let mut trie = Trie::new(true);
        trie.insert(&["foo", "bar"], 42).unwrap();
        trie.insert(&["foo", "baz"], 666).unwrap();
        trie.insert(&["foo"], 7).unwrap();
        trie.insert(&["bam"], 1).unwrap();
        {
            let iter = trie.iter(false, TrieIterReportStyle::BeforeRecursing);
            let paths: Vec<_> = iter.map(|(path, _trie)| path).collect();
            let got_ = paths.iter().map(|v| v.as_slice()).collect::<Vec<_>>();
            let got: &[&[&str]] = got_.as_slice();
            let expect: &[&[&str]] =
                &[&[], &["bam"], &["foo"], &["foo", "bar"], &["foo", "baz"]];
            assert_eq!(got, expect);
        }
        {
            let iter = trie.iter(false, TrieIterReportStyle::AfterRecursing);
            let paths: Vec<_> = iter.map(|(path, _trie)| path).collect();
            let got_ = paths.iter().map(|v| v.as_slice()).collect::<Vec<_>>();
            let got: &[&[&str]] = got_.as_slice();
            let expect: &[&[&str]] =
                &[&["bam"], &["foo", "bar"], &["foo", "baz"], &["foo"], &[]];
            assert_eq!(got, expect);
        }
    }
}
