use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_multipart::Multipart;
use actix_web::{App, Error, HttpResponse, HttpServer, web};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

const UPLOAD_PATH: &str = "/tmp/rest-api/upload";
const PORT: &str = "9000";

#[derive(Serialize, Deserialize)]
struct File {
    name: String,
    time: u64,
    err: String,
}

#[derive(Deserialize)]
struct Download {
    name: String,
}

async fn upload(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    fs::create_dir_all(UPLOAD_PATH)?;
    let mut filename = "".to_string();
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        filename = format!("{} - {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros(), content_type.get_filename().unwrap(), );
        let filepath = format!("{}/{}", UPLOAD_PATH, sanitize_filename::sanitize(&filename));
        // File::create is blocking operation, use thread pool
        let mut f = web::block(|| std::fs::File::create(filepath))
            .await
            .unwrap();
        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use thread pool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    Ok(HttpResponse::Ok().json(&File {
        name: filename,
        time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        err: "".to_string(),
    }))
}

async fn download(info: web::Path<Download>) -> HttpResponse {
    let path = format!("{}/{}", UPLOAD_PATH, info.name.to_string());
    if !Path::new(path.as_str()).exists() {
        return HttpResponse::NotFound().json(&File {
            name: info.name.to_string(),
            time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            err: "file does not exists".to_string(),
        });
    }
    let data = fs::read(path).unwrap();
    HttpResponse::Ok()
        .header("Content-Disposition", format!("form-data; filename={}", info.name.to_string()))
        .body(data)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    println!("starting server at port {}", PORT);
    HttpServer::new(|| {
        App::new()
            .service(
                web::scope("/api")
                    .route("/files/", web::post().to(upload))
                    .route("/files/{name}/", web::get().to(download)),
            )
    })
        .bind(format!("127.0.0.1:{}", PORT))?
        .run()
        .await
}