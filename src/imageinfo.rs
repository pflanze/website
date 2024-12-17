
use std::time::SystemTime;
use std::{collections::HashMap, sync::Mutex};

use anyhow::Result;
use imagesize::{self, ImageSize};
use kstring::KString;
use lazy_static::lazy_static;

use ahtml::{AId, Node, HtmlAllocator};
use chj_util::notime as time;


// Made a stupid cache that simply caches each derived value for 2
// seconds, then realized that it fetches the info in just 50us. But
// then apparently it still helps to slash more than 50us off the
// total request cost (not sure what's going on)? Leaving it in, and
// possibly deserves to be made better via stat based file
// modification checking (but then that should perhaps become
// universal infrastructure, via file monitoring (inotify2)).
lazy_static!{
    static ref IMAGEINFO: Mutex<HashMap<KString, (ImageSize, SystemTime)>> = Default::default();
}
pub fn imageinfo(path: &str) -> Result<ImageSize> {
    // let mut guard = IMAGEINFO.lock().with_context(
    //     || anyhow!("abandoned mutex"))?; // XX todo reinit
    // missing Send prohibits the above. .
    let mut guard = IMAGEINFO.lock().expect(
        "abandoned mutex"); // XX todo reinit
    let key = KString::from_ref(path);
    let now = SystemTime::now();
    if let Some((siz, t)) = guard.get(&key) {
        let d = now.duration_since(*t)?;
        if d.as_secs() <= 2 {
            return Ok(*siz);
        }
    }
    let siz = time!{
        "get imagesize";

        imagesize::size(path)
    }?;
    guard.insert(key, (siz, now));
    Ok(siz)
}

pub fn static_img(html: &HtmlAllocator,
                  path: &str,
                  src: &str,
                  alt: &str,
                  class: Option<&str>
) -> Result<AId<Node>>
{
    let size = imageinfo(path)?;
    let mut atts = html.new_vec();
    atts.push(html.attribute("src", src)?)?;
    atts.push(html.attribute("alt", alt)?)?;
    atts.push(html.attribute("width", size.width)?)?;
    atts.push(html.attribute("height", size.height)?)?;
    if let Some(class) = class {
        atts.push(html.attribute("class", class)?)?;
    }
    html.img(atts.as_slice(), [])
}
