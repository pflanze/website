//! I couldn't find any URI library that does all of these things:

//! - construct URI from parts and serialize it
//! - construct querystring from parts and make it part of URI

//! Also, I may want relative paths, too?

use kstring::KString;

use crate::{ppath::PPath, url_encoding::{url_encode, url_decode, UrlDecodingError}};


// ------------------------------------------------------------------

/// Simple representation of query strings (are nested representations
/// even supported by browsers?). So far just for constructing the
/// serialized representation.
#[derive(Debug)]
pub struct QueryString(Vec<(KString, KString)>);

impl From<&QueryString> for String {
    fn from(q: &QueryString) -> Self {
        let mut s = String::new();
        let mut is_first = true;
        for (k, v) in &q.0 {
            if is_first {
                is_first = false;
            } else {
                s.push('&');
            }
            s.push_str(&url_encode(&k));
            s.push('=');
            s.push_str(&url_encode(&v));
        }
        s
    }
}

pub trait ToVecKeyVal {
    fn to_vec_key_val(self) -> Vec<(KString, KString)>;
}

impl ToVecKeyVal for Vec<(String, String)> {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().map(
            |(k, v)| (KString::from(k), KString::from(v))).collect()
    }
}
impl ToVecKeyVal for Vec<(&str, &str)> {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().map(
            |(k, v)| (KString::from_ref(k), KString::from_ref(v))).collect()
    }
}
impl<const N: usize> ToVecKeyVal for [(KString, KString); N] {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().collect()
    }
}
impl ToVecKeyVal for &[(KString, KString)] {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().cloned().collect()
    }
}
impl ToVecKeyVal for &[(String, String)] {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().map(
            |(k, v)| (KString::from_ref(k), KString::from_ref(v))).collect()
    }
}
impl ToVecKeyVal for &[(&str, &str)] {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().map(
            |(k, v)| (KString::from_ref(k), KString::from_ref(v))).collect()
    }
}
impl<const N: usize> ToVecKeyVal for [(&str, &str); N] {
    fn to_vec_key_val(self) -> Vec<(KString, KString)> {
        self.into_iter().map(
            |(k, v)| (KString::from_ref(k), KString::from_ref(v))).collect()
    }
}


impl QueryString {
    pub fn new(
        keyvals: impl ToVecKeyVal
    ) -> Self {
        Self(keyvals.to_vec_key_val())
    }

    pub fn from_str(s: &str) -> Result<Self, UrlDecodingError> {
        let mut v = Vec::new();
        for partraw in s.split('&') {
            if ! partraw.is_empty() {
                if let Some((key, val)) = partraw.split_once('=') {
                    v.push((url_decode(key)?.into(),
                            url_decode(val)?.into()));
                } else {
                    // XX accept as value-less thing?
                    v.push((url_decode(partraw)?.into(),
                            "".into()));
                }
            }
        }
        Ok(QueryString(v))
    }

    pub fn push(&mut self, keyval: (KString, KString)) {
        self.0.push(keyval);
    }
}

#[cfg(test)]
mod tests1 {
    use super::*;

    #[test]
    fn t_querystring_from_str() {
        fn t(s: &str, q: &[(&str, &str)]) {
            let q1 = QueryString::from_str(s).expect("not to fail");
            let q11: Vec<(&str, &str)> =
                q1.0.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            assert_eq!(&q11, q);
        }
        t("", &[]);
        t("foo", &[("foo", "")]); // XX not sure
        t("foo=1", &[("foo", "1")]);
        t("=1&&2=", &[("", "1"), ("2", "")]);
        t("foo=1&bar=2", &[("foo", "1"), ("bar", "2")]);
        t("foo=1%26&ba%72=%202", &[("foo", "1&"), ("bar", " 2")]);
        // ^ XX is it ok to decode keys, but doing it after & and = splitting?
        // XXX test unicode
    }
}


// ------------------------------------------------------------------

/// A URI without a scheme or authority part; i.e. relative or absolute path
#[derive(Debug)]
pub struct AUriLocal {
    path: PPath<KString>,
    query: Option<QueryString>,
    // todo: fragment
}

impl AUriLocal {
    pub fn new(path: PPath<KString>, query: Option<QueryString>) -> Self {
        Self { path, query }
    }
    pub fn from_str(path: &str, query: Option<QueryString>) -> Self {
        Self { path: PPath::from_str(path), query }
    }
}

impl From<&AUriLocal> for String {
    fn from(a: &AUriLocal) -> Self {
        let mut pathstring = a.path.to_string();
        if let Some(query) = &a.query {
            pathstring.push('?');
            pathstring.push_str(&String::from(query));
        }
        pathstring
    }
}

impl From<AUriLocal> for String {
    fn from(a: AUriLocal) -> Self {
        Self::from(&a)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_1() {
        let uri = AUriLocal::new(
            PPath::from_str("/foo/bar"),
            None
        );
        assert_eq!(String::from(&uri).as_str(), "/foo/bar");
    }

    #[test]
    fn t_2() {
        let q = QueryString::new([("fun", "1"),
                                  ("Motörhead", "C'est bien ça & méchanique = plus!")]);
        let uri = AUriLocal::from_str(
            "/foo///bar/",
            Some(q)
        );
        assert_eq!(
            String::from(&uri).as_str(),
            "/foo/bar/?fun=1&Mot%C3%B6rhead=C%27est%20bien%20%C3%A7a%20%26%20m%C3%A9chanique%20%3D%20plus%21");
    }
}



pub enum AUri {
    Local(AUriLocal),
    // [todo: full URIs with scheme & authority]
}


