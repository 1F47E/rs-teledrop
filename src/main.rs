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


use mime_guess;
use std::env;
use std::fmt::Write;
use std::path::Path;

use reqwest::{multipart, Body, Client};
use serde::{Deserialize, Serialize};

use tokio::fs::File;
use tokio::io;
use tokio_util::codec::{BytesCodec, FramedRead};

use futures::stream::TryStreamExt;

// loaders
use colored::Colorize;
use spinners::{Spinner, Spinners};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};


const APP_NAME: &str = "teledrop";
const CONFIG_NAME: &str = "config";
const API_URL_BASE: &str = "https://api.telegram.org/bot";
const API_SEND_DOCUMENT: &str = "/sendDocument";
const API_GET_FILE: &str = "/getFile";
const FILE_SIZE_LIMIT: u64 = 20_000_000;

// ===== CONFIG
#[derive(Default, Debug, Serialize, Deserialize)]
struct Config {
    bot_token: String,
    chat_id: String,
}
// get api url with token
impl Config {
    fn api_url_send_document(&self) -> String {
        format!(
            "{}{}{}?chat_id={}",
            API_URL_BASE, self.bot_token, API_SEND_DOCUMENT, self.chat_id
        )
    }
    fn api_url_get_file(&self) -> String {
        format!("{}{}{}", API_URL_BASE, self.bot_token, API_GET_FILE)
    }
    fn api_url_file_url(&self, file_path: String) -> String {
        format!("{}/file/{}/{}", API_URL_BASE, self.bot_token, file_path)
    }
}

// ===== API document upload structs

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
    document: TelegramDocument,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramDocument {
    file_id: String,
}

// ===== API file path structs

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
    file_path: String,
}

/// sendDocument telegram bot api
/// https://core.telegram.org/bots/api#senddocument
/// Use this method to send general files. On success, the sent Message is returned. 
/// Bots can currently send files of any type of up to 50 MB in size, this limit may be changed in the future.
/// Because id getFile limit is 20 MB, this is set as the limit for the file size
async fn api_upload_document(filename: &str, url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let file = File::open(filename).await?;

    let file_path = Path::new(filename);
    let file_size = std::fs::metadata(file_path)?.len();

    // check filesize
    if file_size > FILE_SIZE_LIMIT {
        let limit_mb =  FILE_SIZE_LIMIT / 1000000;
        let msg = format!("Filesize is too big. Max size is {} MB", limit_mb);
        println!("{}", msg.red());
        std::process::exit(1);
    }
    // progress bar init
    // let mut downloaded = 0;
    let mut bytes_uploaded: u64 = 0;

    let pb = ProgressBar::new(file_size);
    let template = "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})";
    pb.set_style(
        ProgressStyle::with_template(template)
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            })
            .progress_chars("/-"),
    );


    // another stream with chunks
    let frame = FramedRead::new(file, BytesCodec::new());
    let stream = frame
        .map_ok(move |chunk| {
            bytes_uploaded += chunk.len() as u64;
            pb.set_position(bytes_uploaded);
            if bytes_uploaded == file_size {
                pb.finish_and_clear();
            }
            chunk
        })
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e));

    let body = Body::wrap_stream(stream);

    //make form part of file
    let mime_type = mime_guess::from_path(filename).first_or_octet_stream();
    let part = multipart::Part::stream(body)
        .file_name(filename.to_string())
        .mime_str(mime_type.essence_str())?;

    //create the multipart form
    let form = multipart::Form::new()
        .part("document", part);

    //send request
    let result = client
        .post(url)
        .multipart(form)
        .send()
        .await?
        .text()
        .await?;

    // parse the response and get the file_id
    let mut file_id = String::new();
    let result: Result<TelegramResponseDocument, serde_json::Error> = serde_json::from_str(&result);
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
        },
        Err(err) => {
            // Handle the error
            println!("{} {}", "Error deserializing response:".red(), err);
        }
    }

    Ok(file_id)
}

/// getFile telegram bot api
/// https://core.telegram.org/bots/api#getfile
/// Use this method to get basic information about a file and prepare it for downloading. 
/// For the moment, bots can download files of up to 20MB in size. On success, a File object is returned. 
/// The file can then be downloaded via the link https://api.telegram.org/file/bot<token>/<file_path>, 
/// where <file_path> is taken from the response. 
/// It is guaranteed that the link will be valid for at least 1 hour. 
/// When the link expires, a new one can be requested by calling getFile again.
async fn api_get_file_path(file_id: &str, url: &str) -> Result<String, Box<dyn std::error::Error>> {

    let client = Client::new();
    let request = RequestGetFile {
        file_id: file_id.to_string(),
    };
    let result = client
        .post(url)
        .json(&request)
        .send()
        .await?
        .text()
        .await?;

    // parse the response and get the file_id
    let mut file_path = String::new();
    let result: Result<FileUploadResponse, serde_json::Error> = serde_json::from_str(&result);
    match result {
        Ok(r) => {
            if !r.ok {
                println!("{}", "File path API error".red());
            }
            // check result.document.file_id
            if r.result.file_path == "" {
                println!("{}", "File path API error: file_path not found".red());
            }
            file_path = r.result.file_path;
        },
        Err(err) => {
            println!("{} {}", "Error deserializing response:".red(), err);
        }
    }
    // exit if not found
    if file_path == "" {
        std::process::exit(1);
    }

    Ok(file_path)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ===== CONFIG
    let cfg_result = confy::load(APP_NAME, CONFIG_NAME);
    let cfg: Config = match cfg_result {
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
        println!(
            "Please set up your configuration file at \n\n\"{}\"",
            config_path.unwrap().to_str().unwrap().green()
        );
        return Ok(());
    }

    // // ===== OPEN & READ THE FILE
    // check arg, check the file size and read the contents
    let filename_opt = env::args().nth(1);
    if filename_opt.is_none() {
        println!("{}", "No filename provided".red());
        return Ok(());
    }
    let filename = filename_opt.unwrap();


    // ===== UPLOAD FILE
    let url = cfg.api_url_send_document();
    let upload_res = api_upload_document(&filename, &url);
    let file_id = tokio::runtime::Runtime::new().unwrap().block_on(upload_res).unwrap();
    // create an empty spinner and stop imidiately printing the file_id
    let mut sp = Spinner::new(Spinners::Dots12, "".into());
    let file_id_msg = format!("File ID: {}", file_id);
    sp.stop_and_persist("✔", file_id_msg.into());


    // ===== GET FILE URL
    // start the spinner
    let loading_str = "Loading file URL...";
    sp = Spinner::new(Spinners::Dots12, loading_str.into());
    // do API call
    let api_file_path = cfg.api_url_get_file();
    let file_path_res = api_get_file_path(&file_id, &api_file_path);
    let file_path = tokio::runtime::Runtime::new().unwrap().block_on(file_path_res).unwrap();
    let file_url = cfg.api_url_file_url(file_path);
    // stop the spinner and print the URL
    let file_url_msg = format!("Download URL (valid for 1 hour):\n{}", &file_url.green());
    sp.stop_and_persist("✔", file_url_msg.into());
    Ok(())

}
