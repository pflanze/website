use rouille::Request;
use rouille::input;

/// Get a particular cookie. O(n) with n == number of cookies.
pub fn get_cookie<'r>(request: &'r Request, key: &str) -> Option<&'r str> {
    input::cookies(request).find(|&(n, _)| n == key).map(|(_, v)| v)
}

