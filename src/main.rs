#[macro_use] extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate config;

use std::path::Path;
use std::io::Read;
use hyper::Client;
use hyper::client::RequestBuilder;
use serde_json::Value;
use config::reader::from_file;
use config::types::Config;

const CONFIG_FILE_NAME : &'static str = "coverage_mon_config";

header! { (AuthToken, "auth-token") => [String] }

fn read_config() -> Config {
    let cwd_path = &format!("{}{}", "./", CONFIG_FILE_NAME);
    let cwd_config_file = Path::new(cwd_path);
    if cwd_config_file.exists() {
        return from_file(cwd_config_file).unwrap();
    }

    let home_dir = match std::env::home_dir() {
            Some(dir) => dir,
            None => std::process::exit(-1)
    };

    let home_path = &format!("{}{}", home_dir.to_str().unwrap(), CONFIG_FILE_NAME);
    let home_config_file = Path::new(home_path);
    return from_file(home_config_file).unwrap();
}


fn main() {

    let config = read_config();
    let meta_token = config.lookup_str("meta_token").unwrap();
    let stat_token = config.lookup_str("stat_token").unwrap();

    let client = Client::new();
    let projects = get_projects(&client, meta_token);

    println!("projects ({}) {:?}", projects.len(), projects);

    for project in projects {
        let diff_perc = get_diff_perc(&client, project.as_str(), stat_token);
        if !diff_perc.is_sign_positive() {
            println!("diff {}: {:?}", project, diff_perc);
        }
    }
}

fn get_request<'a>(client: &'a Client, resource: &'a str) -> RequestBuilder<'a> {
    let url : &str = &format!("{}{}", "https://130.211.118.12/", resource);
    return client.get(url);
}

fn meta_get_request<'a>(client: &'a Client, resource: &'a str, token: &'a str) -> RequestBuilder<'a> {
    let req = get_request(client, resource);
    return req.header(AuthToken(token.to_owned()));
}

fn stat_get_request<'a>(client: &'a Client, resource: &'a str, token: &'a str) -> RequestBuilder<'a> {
    let req = get_request(client, resource);
    return req.header(AuthToken(token.to_owned()));
}

fn get_diff_perc<'a>(client: &'a Client, proj: &'a str, token: &'a str) -> f64 {
    let url : &str = &format!("{}{}", "statistics/diff/coverage/", proj);
    let req = stat_get_request(client, url, token);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    return json.as_object().unwrap().get("diff-percentage").unwrap().as_f64().unwrap();
}

fn get_projects<'a>(client: &'a Client, token: &'a str) -> Vec<String> {
    let req = meta_get_request(client, "meta/projects", token);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    let projects = json.as_object().unwrap().get("projects").unwrap().as_array().unwrap();

    return projects.iter().map(|p| p.as_object().unwrap().get("project").unwrap().as_str().unwrap().to_string()).collect();
}
