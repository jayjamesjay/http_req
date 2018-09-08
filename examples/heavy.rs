extern crate http_req;
use http_req::request;

use std::{fs::File, io::Write};

fn main() {
    let res = request::get("https://atom-installer.github.com/v1.30.0/AtomSetup-x64.exe?s=1535142947&ext=.exe").unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
    println!("Headers: {:?}", res.headers());
    
    let mut f = File::create("atom.exe").unwrap();
    f.write_all(&res.body()).unwrap();
}
