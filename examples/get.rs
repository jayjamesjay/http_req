use http_req::request;
use http_req::{request::Request, response::StatusCode, uri::Uri};
use std::convert::TryFrom;
use std::fs::File;

fn main() {
    //Container for body of a response.
    //let mut body = Vec::new();

    //Sends a HTTP GET request and processes the response. Saves body of the response to `body` variable.
    //let res = request::get("https://drivers.amd.com/drivers/installer/23.40/whql/amd-software-adrenalin-edition-24.5.1-minimalsetup-240514_web.exe", &mut body).unwrap();

    //Prints details about the response.
    //println!("Status: {} {}", res.status_code(), res.reason());
    //println!("Headers: {}", res.headers());
    //println!("{}", String::from_utf8_lossy(&body));

    let mut writer = File::create("boo.txt").unwrap();
    let uri = Uri::try_from("https://drivers.amd.com/drivers/whql-amd-software-adrenalin-edition-24.5.1-win10-win11-may15-rdna.exe").unwrap();

    let response = Request::new(&uri)
        .header("Referer", &uri)
        .send(&mut writer)
        .unwrap();

    println!("Status: {} {}", response.status_code(), response.reason());
    println!("Headers: {}", response.headers());
}
