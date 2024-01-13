//! A service that allows to upload and download files in sqlite
//! format, for data exchange with distributed instances.

use std::{sync::Arc, path::{Path, PathBuf}, ffi::OsString};

use anyhow::Result;

use kstring::KString;
use lazy_static::lazy_static;
use rouille::{start_server, Response};
use website::{nav::{Nav, NavEntry, SubEntries},
              ahtml::{AllocatorPool, Allocator},
              webparts::{markdownpage_handler, server_handler, LayoutInterface},
              router::MultiRouter,
              handler::{Handler, FileHandler, FnHandler},
              website_layout::WebsiteLayout,
              path::IntoBoxPath,
              arequest::ARequest,
              ppath::PPath,
              easy_fs::{easy_filepaths_in_dir, FileKind, easy_filenames_in_dir}, try_result, webutils::htmlresponse, http_response_status_codes::HttpResponseStatusCode};

fn files_in_storage(data_files_dir: &Path) -> Result<Vec<OsString>> {
    let mut v: Vec<_> = 
        easy_filenames_in_dir(&*data_files_dir)?
        .filter_map(|r| {
            try_result!{
                let (p, k) = r?;
                Ok(if k == FileKind::File { Some(p) } else { None })
            }.transpose()
        })
        .collect::<Result<_, _>>()?;
    v.sort();
    Ok(v)
}

// A handler that serves an index.md file from there and maybe more,
// and files 
fn files_handler(templates_base: impl IntoBoxPath,
                 data_files_dir: impl IntoBoxPath,
                 style: Arc<dyn LayoutInterface>) -> Arc<dyn Handler> {
    let templates_base = templates_base.into_box_path();
    let data_files_dir = data_files_dir.into_box_path();
    Arc::new(FnHandler(
        move |request: &ARequest, pathrest: &PPath<KString>, html: &Allocator|
                                                   -> Result<Option<Response>> {
            let mut fileitems = html.new_vec();
            for file in files_in_storage(&*data_files_dir)? {
                fileitems.push(
                    html.tr([],
                            [
                                html.td([],
                                        [html.str(&file.to_string_lossy())?])?
                            ])?
                )?;
            }
            Ok(Some(htmlresponse(
                html,
                HttpResponseStatusCode::OK200,
                |html| {
                    style.page(
                        request,
                        html,
                        None,
                        None,
                        None,
                        None,
                        None,
                        html.table([],
                                   fileitems.as_slice())?,
                        None)
                })?))
        }))
}

lazy_static!{
    static ref ALLOCPOOL: AllocatorPool =
        AllocatorPool::new(1000000, true); // XX config
}

fn main() -> Result<()> {

    let style = Arc::new(WebsiteLayout {
        site_name: "SQsync", // "SQlite database file sync service",
        nav: &Nav(&[
            NavEntry {
                name: "Main",
                path: "/",
                subentries: SubEntries::Static(&[]),
            },
        ]),
    });

    let mut router : MultiRouter<Arc<dyn Handler>> = MultiRouter::new();
    router
        .add("/", markdownpage_handler("data-sqservice/index.md", style.clone()))
        // re-use the static files from the other project, uh
        .add("/static", Arc::new(FileHandler::new("data/static")))
        .add("/files", files_handler(
            "data-sqservice/files/",
            "data-sqservice/tmp", // XX
            style.clone()))
        ;


    start_server(
        std::env::var("LISTEN").ok()
            .or_else(|| Some(String::from("127.0.0.1:4000"))).unwrap(),
        server_handler(
            router,
            None,
            &ALLOCPOOL,
            std::io::stdout(),
        ));
}
