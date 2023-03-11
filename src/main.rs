//
//
//       _       _          _                 
//      | |     | |        | |                
//      | |_ ___| | ___  __| |_ __ ___  _ __  
//      | __/ _ \ |/ _ \/ _` | '__/ _ \| '_ \ 
//      | ||  __/ |  __/ (_| | | | (_) | |_) |
//       \__\___|_|\___|\__,_|_|  \___/| .__/ 
//                                     | |    
//                                     |_|    
//
//
//
// Author: Kaspar Industries
// License: MIT
// Description: CLI for Uploading files via telegram
// Dependencies: reqwest, serde, serde_json, confy, dirs, spinners, colored
// Usage: teledrop filename
//
// config file should be found at:
// MacOS: "/Users/user/Library/Application Support/rs.teledrop/config.toml"
// config example:
// 
// bot_token = '123456789:ABC-DEF1234ghIkl-zyx57W2v1u123ew11'
// chat_id = '123456789'
//
use reqwest::blocking::multipart::{Form, Part};
// use reqwest::multipart::{Form, Part};
use reqwest::blocking::Client;
// use reqwest::{Body, ReadCallback};
use reqwest::{Body, Error, StatusCode};
use reqwest::header;

use std::fs::File;
// use std::io::prelude::*;
use std::io::{self, prelude::*};


// use reqwest::header;
use std::env;


use serde::{Serialize, Deserialize};
use spinners::{Spinner, Spinners};
use colored::Colorize;


const APP_NAME: &str = "teledrop";
const CONFIG_NAME: &str = "config";
const API_URL_BASE: &str = "https://api.telegram.org/bot";
const API_SEND_DOCUMENT: &str = "/sendDocument";
const API_GET_FILE: &str = "/getFile";
const FILE_SIZE_LIMMIT: u64 = 20_000_000;
// config file 
#[derive(Default, Debug, Serialize, Deserialize)]
struct Config {
    bot_token: String,
    chat_id: String,
}
// get api url with token
impl Config {
    fn get_api_send_document(&self) -> String {
        format!("{}{}{}?chat_id={}", API_URL_BASE, self.bot_token, API_SEND_DOCUMENT, self.chat_id)
    }
    fn get_api_get_file(&self) -> String {
        format!("{}{}{}", API_URL_BASE, self.bot_token, API_GET_FILE)
    }
    fn get_api_file_url(&self, file_path: String) -> String {
        format!("{}/file/{}/{}", API_URL_BASE, self.bot_token, file_path)
    }
}

// API document upload structs
#[derive(Debug, Serialize, Deserialize)]
struct RequestDocumentUpload {
    document: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramResponseDocument {
    ok: bool,
    result: Option<TelegramResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramResult {
    message_id: i64,
    from: TelegramUser,
    chat: TelegramChat,
    date: i64,
    document: TelegramDocument,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramUser {
    id: i64,
    is_bot: bool,
    first_name: String,
    username: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramChat {
    id: i64,
    first_name: String,
    username: String,
    r#type: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramDocument {
    file_name: String,
    mime_type: String,
    file_id: String,
    file_unique_id: String,
    file_size: i64,
}

// Get file API structs

#[derive(Debug, Serialize, Deserialize)]
struct RequestGetFile {
    file_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileUploadResponse {
    ok: bool,
    result: FileUploadResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileUploadResult {
    file_id: String,
    file_unique_id: String,
    file_size: u64,
    file_path: String,
}



fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ===== CONFIG
    let cfg_result = confy::load(APP_NAME, CONFIG_NAME);
    let cfg:Config = match cfg_result {
        Ok(file) => file,
        Err(error) => {
            println!("{} {}", "Config error:".red(), error);
            return Ok(());
        }
    };
    // check if bot_token and chat_id exists in config
    if cfg.bot_token == "" {
        println!("{}", "Config param bot_token is missing".red());
    }
    if cfg.chat_id == "" {
        println!("{}", "Config param chat_id is missing".red());
    }
    if cfg.bot_token == "" || cfg.chat_id == "" {
        // print config file path
        let config_path = confy::get_configuration_file_path(APP_NAME, CONFIG_NAME);
        println!("Please set up your configuration file at \n\n\"{}\"", config_path.unwrap().to_str().unwrap().green());
        return Ok(());
    }


    // ===== OPEN & READ THE FILE
    // check arg, check the file size and read the contents
    let filename_opt = env::args().nth(1);
    if filename_opt.is_none() {
        println!("{}", "No filename provided".red());
        return Ok(());
    }
    let filename = filename_opt.unwrap();
    let file_opt = File::open(&filename);
    if file_opt.is_err() {
        println!("{}", "File not found".red());
        return Ok(());
    }
    let mut file = file_opt.unwrap();
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    if size > FILE_SIZE_LIMMIT {
        let size_mb = size as f64 / 1_000_000.0;
        println!("{} {}{}", "File size is too big. Max allowed is".red(), size_mb.to_string().red(), "Mb".red());
        return Ok(());
    }

    // Read the contents 
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .expect("Something went wrong reading the file");


    // ===== UPLOADING
    // Create a multipart form with a document parameter containing the binary file
    let form = Form::new()
        .part(
            "document",
            Part::bytes(contents)
            // Part::stream_with_length(contents, size)
                .file_name("filename.bin") // TODO: preserve the file name
                .mime_str("application/octet-stream")?,
        );

    // TODO: make uploading with progress
       // read file body stream
    // let stream = FramedRead::new(file, BytesCodec::new());
    // let file_body = Body::wrap_stream(stream);
    //
    // //make form part of file
    // let some_file = Part::bytes(contents)
    //     .file_name(filename)
    //     .mime_str("text/plain")?;
    //
    // //create the multipart form
    // let form = multipart::Form::new()
    //     .text("username", "seanmonstar")
    //     .text("password", "secret")
    //     .part("file", some_file);

    // upload with progress
    // let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    // let length = file.metadata()?.len();

    // let progress = |uploaded: u64| {
    //     let percent = 100.0 * (uploaded as f64) / (size as f64);
    //     print!("\rUploading {:.2}%", percent);
    //     io::stdout().flush().ok();
    // };
    // let b = Body::wrap_read_with_callback(file, progress);
    // let part = reqwest::blocking::multipart::Part::stream(Body::wrap_read_with_callback(some_file, progress));
    // let part = reqwest::blocking::multipart::Part::stream_with_length(Body::wrap_read_with_callback(some_file, progress), length).file_name(filename);
    // let part = reqwest::multipart::Part::stream_with_length(Body::wrap_read_with_callback(some_file, progress), length).file_name(filename);
    // let part = Part::stream_with_length(Body::wrap_read(file), size);
    // let form = Form::new().part("document", part);


    // start loading 
    let loading_str = format!("{}", "Uploading...".green());
    let mut sp = Spinner::new(Spinners::Dots12, loading_str.into());

    // Send the multipart form to the Telegram API
    let client = Client::new();
    let response = client
        .post(cfg.get_api_send_document())
        .multipart(form)
        .send()?;


    // stop loading
    sp.stop_with_message("".to_string());

    // println!("{}", response.text()?);
    // println!("{}", serde_json::to_string_pretty(&response.text()?)?);

    // print status code
    // println!("{}", response.status());
    // check the status code
    // if !&response.status().is_success() {
    //     println!("{}", "Uploading error".red());
    //     // pretty print json response
    //     // let res: Value = serde_json::from_str(&response.text()?)?;
    //     println!("{}", serde_json::to_string_pretty(&response.text()?)?);
    //
    //     // TODO:get error from description from response
    //     // let result: Result<TelegramResponseDocument, serde_json::Error> = serde_json::from_str(&response.text()?);
    //     return Ok(());
    // }

    // parse the response and get the file_id
    let mut file_id = String::new();
    let result: Result<TelegramResponseDocument, serde_json::Error> = serde_json::from_str(&response.text()?.to_string());
    match result {
        Ok(r) => {
            if !r.ok {
                println!("{}", "Uploading error".red());
            }
            // check result.document.file_id
            if r.result.is_none() {
                println!("{}", "Uploading error".red());
            }
            file_id = r.result.unwrap().document.file_id;
            println!("{} {}", "File ID: ".green(), &file_id);
        },
        Err(err) => {
            // Handle the error
            println!("{} {}", "Error deserializing response:".red(), err);
        }
    }

    // ===== GET FILE PATH
    let req_get_file = RequestGetFile {
        file_id: file_id,
    };
    // serialize to json for post request
    let req_get_file_res = serde_json::to_string(&req_get_file);
    let mut req_get_file_json = String::new();
    match req_get_file_res {
        Ok(_val) => {
            req_get_file_json.push_str(&_val);
        }
        Err(_e) => {
            println!("{}", "Failed to serialize request".red());
            return Ok(());
        }
    }
    // make a get file request 
    let mut headers = header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());

    let response = client
        .post(cfg.get_api_get_file())
        .headers(headers)
        .body(req_get_file_json)
        .send()?;

    let json_str = response.text()?;
    let response: FileUploadResponse = serde_json::from_str(&json_str)?;
    // get file_path
    let file_path = response.result.file_path;
    let download_url = cfg.get_api_file_url(file_path);
    println!("{} {}", "Download URL (valid for 1 hour): ".green(), &download_url);

    Ok(())
}

