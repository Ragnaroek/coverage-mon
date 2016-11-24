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

    info!("coverage_mon started (v0.1)");
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
    display.init().unwrap(); // TODO Proper Err-Logging
    let display_rc = Rc::new(RefCell::new(display));

    info!("coverage_mon init completed");

    loop {
        let project_data = load_project_data(&client, meta_token, stat_token, &excludes);

        for i in 0..project_data.len() {
            let diff = &project_data[i];

            let col = col(i);
            let row = row(i);

            if diff.lines < 0 {
                trellis.set_led(col, row);
            } else {
                trellis.clear_led(col, row);
            }
        }

        trellis.write_display();
        info!("wrote new project state to trellis");

        let evt_start = SystemTime::now();
        let display_ref = display_rc.clone();
        trellis.button_evt_loop(Box::new(move |_trellis:&mut Trellis, evt:ButtonEvent| {
            if evt.buttons_pressed.len() > 0 {
                let first_pressed = evt.buttons_pressed[0];
                let ix = led_index(first_pressed.col, first_pressed.row);

                let mut d = display_ref.borrow_mut();
                if ix < project_data.len() {
                    d.row_select(DisplayRow::R0);
                    d.write_string(project_data[ix].project_name.as_str());
                    d.row_select(DisplayRow::R1);
                    d.write_string(display_coverage(&project_data[ix]).as_str());
                } else {
                    d.row_select(DisplayRow::R0);
                    d.write_string("no project");
                    d.row_select(DisplayRow::R1);
                    d.write_string("");
                }
            }

            let now = SystemTime::now();
            return now.duration_since(evt_start).unwrap() > Duration::from_secs(5*60);
        }));
    }
}

fn display_coverage(diff: &ProjectDiff) -> String {
    return format!("{:+} lines", diff.lines);
}

fn load_project_data(client: &Client, meta_token: &str, stat_token: &str, excludes:&Vec<&str>) -> Vec<ProjectDiff> {
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

    let mut project_data = Vec::with_capacity(projects.len());
    for i in 0..projects.len() {
        let project = &projects[i];
        let data = get_project_data(&client, project.as_str(), stat_token);
        project_data.insert(i, data);
    }
    return project_data;
}

// TODO make functions in trellis public
fn led_index(col:Col, row:Row) -> usize {
    return (row_to_num(row)*4 + col_to_num(col)) as usize;
}

fn row_to_num(row: Row) -> u8 {
    match row {
        Row::R0 => 0,
        Row::R1 => 1,
        Row::R2 => 2,
        Row::R3 => 3
    }
}

fn col_to_num(col: Col) -> u8 {
    match col {
        Col::A => 0,
        Col::B => 1,
        Col::C => 2,
        Col::D => 3,
    }
}

// TODO put functions to trellis lib
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

struct ProjectDiff {
    project_name: String,
    lines: i64,
}

fn get_project_data(client: &Client, proj: &str, token: &str) -> ProjectDiff {
    let url : &str = &format!("{}{}", "statistics/diff/coverage/", proj);
    let req = stat_get_request(client, url, token);
    let mut response = req.send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();
    let json_diff = json.as_object().unwrap();
    let lines = json_diff.get("diff-lines").unwrap().as_i64().unwrap();
    return ProjectDiff{project_name: String::from(proj), lines: lines};
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
