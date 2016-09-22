#[macro_use] extern crate hyper;
extern crate serde;
extern crate serde_json;

use std::io::Read;
use hyper::Client;
use hyper::client::RequestBuilder;
use serde_json::Value;

// TODO make token configurable via config file
const META_TOKEN : &'static str = "<todo>";
const STAT_TOKEN : &'static str = "<todo>";

header! { (AuthToken, "auth-token") => [String] }

fn main() {
    let client = Client::new();
    let projects = get_projects(&client);

    println!("projects ({}) {:?}", projects.len(), projects);

    // TODO use stat_token for this request to work!!!!!
    println!("diff0 {:?}", get_diff_perc(&client, projects[0].as_str()));
}

fn get_request<'a>(client: &'a Client, resource: &'a str) -> RequestBuilder<'a> {
    let url : &str = &format!("{}{}", "https://130.211.118.12/", resource);
    let res = client.get(url)
        .header(AuthToken(META_TOKEN.to_owned()));
    return res;
}

fn get_diff_perc<'a>(client: &'a Client, proj: &'a str) -> f64 {
    let url : &str = &format!("{}{}", "statistics/coverage/diff/", proj);
    let req = get_request(client, url);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    return json.as_object().unwrap().get("diff-percentage").unwrap().as_f64().unwrap();
}

fn get_projects<'a>(client: &'a Client) -> Vec<String> {
    let req = get_request(client, "meta/projects");
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    let projects = json.as_object().unwrap().get("projects").unwrap().as_array().unwrap();

    return projects.iter().map(|p| p.as_object().unwrap().get("project").unwrap().as_str().unwrap().to_string()).collect();
}
