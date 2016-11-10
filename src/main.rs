#[macro_use] extern crate hyper;
#[macro_use] extern crate log;
extern crate serde;
extern crate serde_json;
extern crate config;
extern crate env_logger;
extern crate trellis;
extern crate hd44780;

use std::path::Path;
use std::io::Read;
use std::boxed::Box;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, SystemTime};
use hyper::Client;
use hyper::client::RequestBuilder;
use serde_json::Value;
use config::reader::from_file;
use config::types::Config;

use trellis::core::{Trellis, Col, Row, ButtonEvent};

use hd44780::core::HD44780;
use hd44780::core::DisplayRow;

const CONFIG_FILE_NAME : &'static str = "coverage_mon_config";

header! { (AuthToken, "auth-token") => [String] }

fn val_to_str<'a>(val: &'a config::types::Value) -> &'a str {
    return match val {
        &config::types::Value::Svalue(config::types::ScalarValue::Str(ref s)) => s.as_str(),
        _ => panic!()
    };
}

fn main() {

    info!("coverage_mon started");
    env_logger::init().unwrap();

    let config = read_config();
    let meta_token = config.lookup_str("meta_token").unwrap();
    let stat_token = config.lookup_str("stat_token").unwrap();
    let excludes_raw = config.lookup("exclude_projects").and_then(
        |v| match v {
            &config::types::Value::Array(ref v) => Some(v),
            _ => None
        }).unwrap();
    let excludes:Vec<&str> = excludes_raw.iter().map(|v| val_to_str(v)).collect();

    let client = Client::new();

    let pi_dev = trellis::devices::RaspberryPiBPlus::new();
    let mut trellis = Trellis::new(Box::new(pi_dev));
    trellis.init();

    let host = hd44780::hosts::RaspberryPiBPlus::new();
    let mut display = HD44780::new(Box::new(host));
    let display_ref = Rc::new(RefCell::new(display));

    info!("coverage_mon init completed");

    loop {
        info!("checking project state");
        let all_projects = get_projects(&client, meta_token);
        let mut filtered:Vec<String> = all_projects.into_iter().filter(|x| !excludes.contains(&x.as_str())).collect();
        filtered.sort();

        if filtered.len() > 16 {
            warn!("more than 16 projects, only the first 16 will be shown");
        }
        filtered.truncate(16);
        let projects = filtered.to_vec();
        info!("checking coverage change for projects {:?}", projects);

        for i in 0..projects.len() {
            let project = &projects[i];
            let diff_perc = get_diff_perc(&client, project.as_str(), stat_token);

            let col = col(i);
            let row = row(i);

            if !diff_perc.is_sign_positive() {
                trellis.set_led(col, row);
            } else {
                trellis.clear_led(col, row);
            }
        }

        trellis.write_display();
        info!("wrote new project state to trellis");

        // TODO RefCell for display???
        let evt_start = SystemTime::now();

        let cb = Box::new(move |trellis:&mut Trellis, evt:ButtonEvent| {
            if evt.buttons_pressed.len() > 0 {
                let mut d = display_ref.borrow_mut();
                d.row_select(DisplayRow::R0);
                d.write_string("test");
            }

            let now = SystemTime::now();
            return now.duration_since(evt_start).unwrap() > Duration::from_secs(3);
        });
        trellis.button_evt_loop(cb);
    }
}

fn num_to_col(num: usize) -> Col {
    match num {
        0 => Col::A,
        1 => Col::B,
        2 => Col::C,
        3 => Col::D,
        _ => panic!("illegal column")
    }
}

fn num_to_row(num: usize) -> Row {
    match num {
        0 => Row::R0,
        1 => Row::R1,
        2 => Row::R2,
        3 => Row::R3,
        _ => panic!("illegal row")
    }
}

// TODO put next two functions in trellis lib!
fn col(num: usize) -> Col {
    let col_num = num % 4;
    return num_to_col(col_num);
}

fn row(num: usize) -> Row {
    let row_num = num / 4;
    return num_to_row(row_num);
}

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

fn get_request<'a>(client: &'a Client, resource: &str) -> RequestBuilder<'a> {
    let url: &str = &format!("{}{}", "https://130.211.118.12/", resource);
    return client.get(url);
}

fn meta_get_request<'a>(client: &'a Client, resource: &str, token: &str) -> RequestBuilder<'a> {
    let req = get_request(client, resource);
    return req.header(AuthToken(token.to_owned()));
}

fn stat_get_request<'a>(client: &'a Client, resource: &str, token: &str) -> RequestBuilder<'a> {
    let req = get_request(client, resource);
    return req.header(AuthToken(token.to_owned()));
}

fn get_diff_perc(client: &Client, proj: &str, token: &str) -> f64 {
    let url : &str = &format!("{}{}", "statistics/diff/coverage/", proj);
    let req = stat_get_request(client, url, token);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    return json.as_object().unwrap().get("diff-percentage").unwrap().as_f64().unwrap();
}

fn get_projects(client: &Client, token: &str) -> Vec<String> {
    let req = meta_get_request(client, "meta/projects", token);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    let projects = json.as_object().unwrap().get("projects").unwrap().as_array().unwrap();

    return projects.iter().map(|p| p.as_object().unwrap().get("project").unwrap().as_str().unwrap().to_string()).collect();
}
