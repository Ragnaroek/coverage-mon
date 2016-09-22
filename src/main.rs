#[macro_use] extern crate hyper;
extern crate rustc_serialize;

use std::io::Read;
use hyper::Client;
use hyper::client::RequestBuilder;
use rustc_serialize::json::Json;
use std::collections::BTreeSet;

// TODO make token configurable via config file
const AUTH_TOKEN : &'static str = "<insert-token-here>";

header! { (AuthToken, "auth-token") => [String] }

fn main() {
    // Create a client.
    let client = Client::new();

    // Creating an outgoing request.
    let mut res = client.get("https://130.211.118.12/meta/projects")
        // set a header
        .header(AuthToken(AUTH_TOKEN.to_owned()))
        // let 'er go!
        .send().unwrap();

    // Read the Response.
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();

    let response = Json::from_str(&body).unwrap();
    let data = response.as_object().unwrap().get("projects").unwrap().as_array().unwrap();

    let mut result = BTreeSet::new();
    for project in data {
        let name = project.as_object().unwrap().get("project").unwrap().as_string().unwrap();
        result.insert(name);
    }

    println!("projects ({}) {:?}", result.len(), result);
}

fn get_request<'a>(client: &'a Client, resource: &'a str) -> RequestBuilder<'a> {
    let url : &str = &format!("{}{}", "https://130.211.118.12/", resource);
    let res = client.get(url)
        .header(AuthToken(AUTH_TOKEN.to_owned()));
    return res;
}

fn get_projects<'a>(client: &'a Client) {
    let req = get_request(client, "meta/projects");
    println!("result {:?}", req.send().unwrap());
}
