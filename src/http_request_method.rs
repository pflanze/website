
//! Pattern matching and processing help for HTTP request methods.

// https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestMethod {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestMethodSimple {
    GET,
    HEAD,
    POST,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestMethodDocument {
    PUT,
    DELETE,
    PATCH,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestMethodSpecial {
    OPTIONS,
    TRACE,
    CONNECT,
}

pub enum HttpRequestMethodGrouped {
    Simple(HttpRequestMethodSimple),
    Document(HttpRequestMethodDocument),
    Special(HttpRequestMethodSpecial)
}

// --------------------------------------------

impl HttpRequestMethodSimple {
    pub fn is_post(self) -> bool {
        match self {
            HttpRequestMethodSimple::GET => false,
            HttpRequestMethodSimple::HEAD => false,
            HttpRequestMethodSimple::POST => true
        }
    }
    pub fn to_http_request_method(self) -> HttpRequestMethod {
        match self {
            HttpRequestMethodSimple::GET => HttpRequestMethod::GET,
            HttpRequestMethodSimple::HEAD => HttpRequestMethod::HEAD,
            HttpRequestMethodSimple::POST => HttpRequestMethod::POST
        }
    }
}

impl HttpRequestMethodDocument {
    pub fn to_http_request_method(self) -> HttpRequestMethod {
        match self {
            HttpRequestMethodDocument::PUT => HttpRequestMethod::PUT,
            HttpRequestMethodDocument::DELETE => HttpRequestMethod::DELETE,
            HttpRequestMethodDocument::PATCH => HttpRequestMethod::PATCH,
        }
    }
}

impl HttpRequestMethodSpecial {
    pub fn to_http_request_method(self) -> HttpRequestMethod {
        match self {
            HttpRequestMethodSpecial::OPTIONS => HttpRequestMethod::OPTIONS,
            HttpRequestMethodSpecial::TRACE => HttpRequestMethod::TRACE,
            HttpRequestMethodSpecial::CONNECT => HttpRequestMethod::CONNECT,
        }
    }
}

impl HttpRequestMethodGrouped {
    pub fn to_http_request_method(self) -> HttpRequestMethod {
        match self {
            HttpRequestMethodGrouped::Simple(v) => v.to_http_request_method(),
            HttpRequestMethodGrouped::Document(v) => v.to_http_request_method(),
            HttpRequestMethodGrouped::Special(v) => v.to_http_request_method(),
        }
    }
}

impl HttpRequestMethod {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "GET" => Ok(Self::GET),
            "HEAD" => Ok(Self::HEAD),
            "POST" => Ok(Self::POST),
            "PUT" => Ok(Self::PUT),
            "PATCH" => Ok(Self::PATCH),
            "DELETE" => Ok(Self::DELETE),
            "OPTIONS" => Ok(Self::OPTIONS),
            "CONNECT" => Ok(Self::CONNECT),
            "TRACE" => Ok(Self::TRACE),
            _ => bail!("invalid http request method {s:?}")
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::HEAD => "HEAD",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::PATCH => "PATCH",
            Self::DELETE => "DELETE",
            Self::OPTIONS => "OPTIONS",
            Self::CONNECT => "CONNECT",
            Self::TRACE => "TRACE",
        }
    }

    pub fn is_post(self) -> bool {
        match self {
            Self::POST => true,
            _ => false
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::GET => 
                "The GET method requests a representation of the specified resource. \
                 Requests using GET should only retrieve data.",
            Self::HEAD =>
                "The HEAD method asks for a response identical to a GET request, \
                 but without the response body.",
            Self::POST =>
                "The POST method submits an entity to the specified resource, often \
                 causing a change in state or side effects on the server.",

            Self::PUT =>
                "The PUT method replaces all current representations of the target \
                 resource with the request payload.",
            Self::PATCH =>
                "The PATCH method applies partial modifications to a resource.",
            Self::DELETE =>
                "The DELETE method deletes the specified resource.",

            Self::OPTIONS =>
                "The OPTIONS method describes the communication options for the target resource.",

            Self::CONNECT =>
                "The CONNECT method establishes a tunnel to the server identified by \
                 the target resource.",
            Self::TRACE =>
                "The TRACE method performs a message loop-back test along the path to \
                 the target resource.",
        }
    }

    pub fn to_grouped(self) -> HttpRequestMethodGrouped {
        match self {
            Self::GET =>
                HttpRequestMethodGrouped::Simple(HttpRequestMethodSimple::GET),
            Self::HEAD =>
                HttpRequestMethodGrouped::Simple(HttpRequestMethodSimple::HEAD),
            Self::POST =>
                HttpRequestMethodGrouped::Simple(HttpRequestMethodSimple::POST),

            Self::PUT =>
                HttpRequestMethodGrouped::Document(HttpRequestMethodDocument::PUT),
            Self::PATCH =>
                HttpRequestMethodGrouped::Document(HttpRequestMethodDocument::PATCH),
            Self::DELETE =>
                HttpRequestMethodGrouped::Document(HttpRequestMethodDocument::DELETE),

            Self::OPTIONS =>
                HttpRequestMethodGrouped::Special(HttpRequestMethodSpecial::OPTIONS),
            Self::CONNECT =>
                HttpRequestMethodGrouped::Special(HttpRequestMethodSpecial::CONNECT),
            Self::TRACE =>
                HttpRequestMethodGrouped::Special(HttpRequestMethodSpecial::TRACE),
        }
    }
}

