use flate2::write::GzEncoder;
use flate2::Compression;
use itertools::Itertools;
use nom::AsBytes;
use std::fs;
use std::fs::read_to_string;
use std::io::Write;
use std::u32;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task;

async fn handle_post(mut stream: TcpStream, stringbuf: &str, checkstr: &str) {
    match checkstr {
        _ if checkstr.starts_with("/files") => {
            let filename = &checkstr[7..];
            println!("{filename}");
            let content_type_temp = &stringbuf[stringbuf.find("Content-Type:").unwrap()..];
            let content_type = &content_type_temp[14..content_type_temp.find("\r").unwrap()];
            println!("{content_type}");
            match content_type {
                "application/octet-stream" | "plain/text" => {
                    let content_len_temp = &stringbuf[stringbuf.find("Content-Length:").unwrap()..];
                    let content_len_str =
                        &content_len_temp[15..content_len_temp.find("\r").unwrap()];
                    println!("content_len_str:{content_len_str}");
                    let content_len_int: u32 = content_len_str.trim().parse().unwrap();
                    println!("{content_len_int}");
                    let body = &content_len_temp[content_len_temp.find("\r\n\r\n").unwrap() + 4..];
                    println!("{body}");
                    let mut write_file = fs::File::create(filename).unwrap();
                    write_file
                        .write(body[..(content_len_int as usize)].as_bytes())
                        .expect("Writing to file didnt work");
                    stream
                        .write_all("HTTP/1.1 201 Created\r\n\r\n".as_bytes())
                        .await
                        .unwrap();
                }
                _ => {
                    println!("Not supported");
                }
            }
        }
        _ => {
            println!("POST INVALID");
        }
    }
}

async fn handle_connection(mut stream: TcpStream) {
    let stringbuf: &str;
    let mut buf: [u8; 1024] = [0; 1024];

    let read_size = stream.read(&mut buf).await.unwrap();
    if read_size == 0 {
        println!("Connection closed");
        return;
    }
    stringbuf = std::str::from_utf8(&buf).unwrap();
    let fromword = stringbuf.find("/").unwrap();
    let tillword = stringbuf.find("HTTP").unwrap();
    let checkstr = &stringbuf[fromword..tillword - 1];
    match stringbuf {
        _ if stringbuf.starts_with("GET") => match checkstr {
            "/" => {
                let current_dir = fs::read_dir(".").unwrap();
                let mut files_vec: Vec<String> = Vec::new();
                for entry in current_dir {
                    let name = entry.unwrap();
                    let filename = name.file_name().into_string().unwrap();
                    files_vec.push(filename.clone());
                    println!("{filename}");
                }
                let all_file_names = files_vec.join("\n");
                let len = all_file_names.len();
                let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{all_file_names}");
                stream.write_all(stringwrite.as_bytes()).await.unwrap();
                stream
                    .write_all("HTTP/1.1 200 OK\r\n\r\n".as_bytes())
                    .await
                    .unwrap();
                println!("its only /");
            }
            _ if checkstr.starts_with("/echo") => {
                let word = &checkstr[6..];
                let mut len = word.len();
                let mut encoding: &str;
                match stringbuf.find("Accept-Encoding: ") {
                    None => {
                        encoding = "None";
                        println!("{encoding}");
                        let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{word}");
                        stream.write_all(stringwrite.as_bytes()).await.unwrap();
                    }
                    _ => {
                        encoding = &stringbuf[stringbuf.find("Accept-Encoding: ").unwrap() + 17..];
                        encoding = &encoding[..encoding.find("\r").unwrap()];
                        let mut encoding_vec = encoding.split_terminator(", ").collect_vec();
                        if encoding_vec.len() == 1 {
                            encoding_vec = encoding.split_terminator(",").collect_vec();
                        }
                        for enc in encoding_vec {
                            if enc == "gzip" {
                                encoding = "gzip";
                            }
                        }
                        println!("{encoding}");
                        match encoding {
                            "gzip" => {
                                let mut encoder =
                                    GzEncoder::new(Vec::new(), Compression::default());
                                encoder
                                    .write_all(word.as_bytes())
                                    .expect("Gzip Compression Failed");
                                let encoded_word = encoder.finish().unwrap();
                                let mut encoded_word_slice = encoded_word.as_slice();
                                encoded_word_slice = encoded_word_slice.as_bytes();
                                len = encoded_word_slice.len();
                                let mut stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: {encoding}\r\nContent-Length: {len}\r\n\r\n").into_bytes();
                                stringwrite.extend_from_slice(&encoded_word_slice);
                                stream.write_all(stringwrite.as_bytes()).await.unwrap();
                            }
                            _ => {
                                let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{word}");
                                stream.write_all(stringwrite.as_bytes()).await.unwrap();
                            }
                        }
                    }
                }
                println!("its echo {word}");
            }
            _ if checkstr.starts_with("/user-agent") => {
                let user_agent_temp = &stringbuf[stringbuf.find("User-Agent:").unwrap()..];
                let user_agent = &user_agent_temp[12..user_agent_temp.find("\r").unwrap()];
                let ua_len = user_agent.len();
                let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {ua_len}\r\n\r\n{user_agent}");
                stream.write_all(stringwrite.as_bytes()).await.unwrap();
                println!("{user_agent}");
            }
            _ if checkstr.starts_with("/files") => {
                let filename = &checkstr[7..];
                // let envargs = env::args().collect_vec();
                // let mut filepath = envargs[2].clone();
                let filepath = filename;
                println!("{filepath}");

                match fs::metadata(filepath) {
                    Err(_e) => {
                        stream
                            .write("HTTP/1.1 404 Not Found\r\n\r\n".as_bytes())
                            .await
                            .unwrap();
                        println!("File not found");
                    }
                    _ if fs::metadata(filepath).unwrap().is_dir() == true => {
                        let current_dir = fs::read_dir(filepath).unwrap();
                        let mut files_vec: Vec<String> = Vec::new();
                        for entry in current_dir {
                            let name = entry.unwrap();
                            let filename = name.file_name().into_string().unwrap();
                            files_vec.push(filename.clone());
                            println!("{filename}");
                        }
                        let all_file_names = files_vec.join("\n");
                        let len = all_file_names.len();
                        let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{all_file_names}");
                        stream.write_all(stringwrite.as_bytes()).await.unwrap();
                        stream
                            .write_all("HTTP/1.1 200 OK\r\n\r\n".as_bytes())
                            .await
                            .unwrap();
                        println!("Is directory");
                    }
                    _ => match read_to_string(filepath) {
                        Ok(file_buf) => {
                            let fb_len = file_buf.len();
                            let stringwrite= format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {fb_len}\r\n\r\n{file_buf}");
                            stream.write_all(stringwrite.as_bytes()).await.unwrap();
                            stream
                                .write_all("HTTP/1.1 200 OK\r\n\r\n".as_bytes())
                                .await
                                .unwrap();
                        }
                        Err(_e) => {}
                    },
                }
            }
            _ => {
                stream
                    .write("HTTP/1.1 404 Not Found\r\n\r\n".as_bytes())
                    .await
                    .unwrap();
                println!("Nope");
            }
        },

        _ if stringbuf.starts_with("POST") => {
            handle_post(stream, stringbuf, checkstr).await;
        }
        _ => {
            println!("INVALID");
        }
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").await.unwrap();
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                task::spawn(handle_connection(stream));
            }
            Err(_e) => {
                println!("Error in connecting");
            }
        }
    }
}
