use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{Response, StatusCode};
use axum::response::Html;
use axum::Router;
use axum::routing::get;
use chrono::{DateTime, Utc};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
    title: String,
    body: String,
    image_url: String,
    summary: String,
    timestamp: DateTime<Utc>,
    tags: Vec<String>,
    #[serde(skip)]
    url_name: String,
}

type FileCache = Arc<Mutex<HashMap<String, Vec<u8>>>>;

fn list_files_in_directory(dir: &str) -> Vec<String> {
    let path = std::path::Path::new(dir);

    // Ensure the directory exists
    if !path.is_dir() {
        println!("Directory {} does not exist.", dir);
        return vec![];
    }

    // Collect file names into a Vec<String>
    let mut file_list = Vec::new();
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    // Check if it's a file (not a directory)
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
                            // Get file name as a String
                            if let Some(file_name) = entry.file_name().to_str() {
                                file_list.push(file_name.to_string());
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("Error reading directory {}: {}", dir, e);
        }
    }

    file_list
}

/// Converts Markdown text to HTML for use in a Maud template
fn markdown_to_html(markdown_text: &str) -> Markup {
    let options = Options::empty();
    let parser = Parser::new_ext(markdown_text, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    PreEscaped(html_output)
}

/// Renders the post in a Maud template, converting the body from Markdown to HTML
fn render_post(post: &Post) -> Markup {
    html! {
        div class="post" {
            h1 { (post.title) }
            p class="text-muted" { (post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()) }
            div class="post-content" {
                (markdown_to_html(&post.body))
            }
        }
    }
}

async fn load_file(filename: &str, cache: FileCache) -> Option<Vec<u8>> {
    let filepath = format!("./caden-blog/assets/{}", filename);
    let mut file = File::open(&filepath).ok()?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).ok()?;

    // Cache the file contents
    cache.lock().expect("cdn falied to lock the cache").insert(filename.to_string(), contents.clone());
    Some(contents)
}

fn serialize_post(post: &Post) -> String {
    serde_json::to_string(post).expect("Failed to serialize Post")
}

fn deserialize_post(json_data: &str,url_name: &str) -> Post {
    let mut post: Post = serde_json::from_str(json_data).expect("Failed to deserialize Post");
    post.url_name = url_name.to_string();
    post
}

fn cache_control_response(content: Vec<u8>) -> Response<Body> {
    use hyper::header::{CACHE_CONTROL, HeaderValue};

    Response::builder()
        .header(CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000"))
        .body(Body::from(content))
        .unwrap()
}

async fn handle_asset_request(Path(filename): Path<String>, cache: FileCache) -> Result<Response<Body>, StatusCode> {
    println!("{}", &filename);
    // Check if file is already cached
    if let Some(content) = cache.lock().expect("cdn failed to lock the cache").get(&filename).cloned() {
        return Ok(cache_control_response(content));
    }

    // Load the file and cache it if not already cached
    if let Some(content) = load_file(&filename, cache.clone()).await {
        Ok(cache_control_response(content))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[tokio::main]
async fn main() {
    let cache: FileCache = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/", get(home))
        .route("/post/:url_name", get(post_handler))
        .route("/posts", get(posts))
        .route("/assets/:filename", get({
            let cache = cache.clone();
            move |path| handle_asset_request(path, cache.clone())
        }))
        .route("/favicon.ico", get(serve_favicon));;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening to {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn serve_favicon() -> Result<Response<Body>, StatusCode> {
    let path = PathBuf::from("./caden-blog/favicon.ico");

    // Try to open the file
    let mut file = File::open(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    let mut contents = Vec::new();

    // Read the file contents into a buffer
    file.read_to_end(&mut contents).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create and return the response with caching headers
    Ok(Response::builder()
        .header("Content-Type", "image/x-icon")
        .header("Cache-Control", "public, max-age=31536000")
        .body(Body::from(contents))
        .unwrap())
}

fn get_from_file(file_name: &str) -> Option<Post> {
    let dir = format!("./caden-blog/posts/{}",file_name);
    let path = std::path::Path::new((&dir).into());
    let display = path.display();
    // println!("{} {}", path.exists(), display.to_string());
    if path.exists() && !display.to_string().contains("..") {
        // Open the path in read-only mode, returns `io::Result<File>`
        let mut file = match File::open(&path) {
            Err(why) => panic!("couldn't open {}: {}", display, why),
            Ok(file) => file,
        };

        let mut post_string = String::new();
        match file.read_to_string(&mut post_string) {
            Err(why) => panic!("couldn't read {}: {}", display, why),
            _ => {}
        }
        Some(deserialize_post(post_string.as_mut_str(), file_name.replace(".json","").as_mut_str()))
    } else {
        None
    }
}

async fn posts(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    let maybetag = params.get("tag");
    let mut posts: Vec<Post> = vec![];
    for file in list_files_in_directory("./caden-blog/posts") {
        posts.push(get_from_file(&file).unwrap());
        //println!("{}", file);
    }
    match maybetag {
        Some(tag) => Html(html! {
        @for post in posts.iter().filter(|v| v.tags.contains(tag)).collect::<Vec<&Post>>() {
            div class="card post-card" {
                img src=(post.image_url) class="card-img-top" alt="Post Image";
                div class="card-body" {
                    h5 class="card-title" { (post.title) }
                    p class="text-muted" { (format!("Posted on {}", post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()))}
                    p class="card-text" { (post.summary) }
                    a href=(format!("/post/{}",post.url_name)) class="btn btn-primary" { "Read More" }
                }
            }
        }
        }.into_string()),
        None => Html(html! {
        @for post in posts {
            div class="card post-card" {
                img src=(post.image_url) class="card-img-top" alt="Post Image";
                div class="card-body" {
                    h5 class="card-title" { (post.title) }
                    p class="text-muted" { (format!("Posted on {}", post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()))}
                    p class="card-text" { (post.summary) }
                    a href=(format!("/post/{}",post.url_name)) class="btn btn-primary" { "Read More" }
                }
            }
        }
        }.into_string())
    }
}

async fn home() -> Html<String> {
    Html(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Res techinca fortuitae" }
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/unpoly@3.9.3/unpoly.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/unpoly@3.9.3/unpoly-bootstrap5.min.css";
                style { r#"
                    body {
                        font-family: Arial, sans-serif;
                        background-color: #121212;
                        color: #e0e0e0;
                    }
                    .header {
                        background-image: url('https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Fpreview.redd.it%2Fi0h9ke187tk31.png%3Fwidth%3D960%26crop%3Dsmart%26auto%3Dwebp%26s%3Ddc294c8327d576f78d3cd0e08982cd6e3f619a21&f=1&nofb=1&ipt=47a8aff3e3499390c872b22b77ba3ad02b9f28fc0c0f5b5d3d82c84dd16ed6a6&ipo=images');
                        background-position: center;
                        color: #f0f0f0;
                        padding: 20px;
                        text-align: center;
                        background-size: cover;
                    }
                    .post-card {
                        background-color: #1e1e1e;
                        color: #e0e0e0;
                        border: none;
                        margin-bottom: 20px;
                        box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                        transition: 0.3s;
                    }
                    .post-card:hover {
                        box-shadow: 0 8px 16px rgba(0, 0, 0, 0.5);
                    }
                    .sidebar {
                        background-color: #242424;
                        color: #e0e0e0;
                        padding: 20px;
                        border-radius: 8px;
                    }
                    .footer {
                        background-color: #1c1c1c;
                        color: #f0f0f0;
                        text-align: center;
                        padding: 15px;
                        margin-top: 20px;
                    }
                    .navbar-nav .nav-link {
                        color: #e0e0e0 !important;
                    }
                    .tag {
                        --bs-btn-color: #7d7d7d;
                    }
                    .btn-primary {
                        background-color: #007bff;
                        border-color: #007bff;
                    }
                    .btn-outline-primary {
                        color: #007bff;
                        border-color: #007bff;
                    }
                    .btn-outline-primary:hover {
                        background-color: #007bff;
                        color: #fff;
                    }
                    .text-muted {
                        color: rgba(101, 106, 111, 0.75) !important;
                    }
                "# }
            }
            body {
                // Header
                div class="header" {
                    h1 { "Res techinca fortuitae" }
                }
                // Navigation Bar
                nav class="navbar navbar-dark bg-dark" {
                    div class="container-fluid" {
                        a class="navbar-brand" { "Posts" }
                        div class="collapse navbar-collapse" id="navbarNav" {
                            // ul class="navbar-nav ms-auto" {
                            //     li class="nav-item" {
                            //         a class="nav-link active" href="#" { "Home" }
                            //     }
                            //     li class="nav-item" {
                            //         a class="nav-link" href="#" { "About" }
                            //     }
                            //     li class="nav-item" {
                            //         a class="nav-link" href="/contact" up-layer="new" { "Contact" }
                            //     }
                            // }
                        }
                    }
                }

                // Main Content
                div class="container my-4" {
                    div class="row" {
                        // Blog Posts
                        div id="content" class="col-lg-8" hx-get="/posts" hx-trigger="load" hx-swap="innerHTML" {}

                        // Sidebar
                        div class="col-lg-4" {
                            div class="sidebar" {
                                h4 { "About Me" }
                                p { "I'm a hacker man :)))." }
                                hr;
                                h5 { "Categories" }
                                ul class="list-unstyled" {
                                    li { a class = "btn tag" hx-vals=r#"{"tag": "hardware"}"# hx-get="/posts" hx-target="#content" hx-swap="innerHTML" { "Hardware" } }
                                    li { a class = "btn tag" hx-vals=r#"{"tag": "software"}"# hx-get="/posts" hx-target="#content" hx-swap="innerHTML" { "Software" } }
                                    li { a class = "btn tag" hx-vals=r#"{"tag": "gaming"}"# hx-get="/posts" hx-target="#content" hx-swap="innerHTML" { "Gaming" } }
                                    li { a class = "btn tag" hx-vals=r#"{"tag": "science"}"# hx-get="/posts" hx-target="#content" hx-swap="innerHTML" { "Science" } }
                                    li { a class = "btn tag" hx-get="/posts" hx-target="#content" hx-swap="innerHTML" { "All" } }
                                }
                                hr;
                            }
                        }
                    }
                }

                // Footer
                div class="footer" {
                    p { "©2024 Res techinca fortuitae | Designed by Caden Ream" }
                }

                script src="https://code.jquery.com/jquery-3.5.1.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/unpoly@3.9.3/unpoly.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/unpoly@3.9.3/unpoly-bootstrap5.min.js" {}
                script src="https://unpkg.com/htmx.org@1.9.12" {}
            }
        }
    }.into_string())
}

async fn post_handler(Path(url_name): Path<String>) -> Html<String> {
    let dir = format!("./caden-blog/posts/{}.json",url_name);
    let path = std::path::Path::new((&dir).into());
    let display = path.display();
    //println!("{} {}", path.exists(), display.to_string());
    if path.exists() && !display.to_string().contains("..") {
        // Open the path in read-only mode, returns `io::Result<File>`
        let mut file = match File::open(&path) {
            Err(why) => panic!("couldn't open {}: {}", display, why),
            Ok(file) => file,
        };

        let mut post_string = String::new();
        match file.read_to_string(&mut post_string) {
            Err(why) => panic!("couldn't read {}: {}", display, why),
            _ => {}
        }
        let mut post = deserialize_post(post_string.as_mut_str(),url_name.as_str());

        let rendered_html = html! {
            (maud::DOCTYPE)
            html data-bs-theme="dark" lang="en" {
                head {
                    script src="https://cdn.jsdelivr.net/gh/MarketingPipeline/Markdown-Tag/markdown-tag.js" {}
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    title { (post.title) }
                    link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                    style { r#"
                        github-md {
                            --color-prettylights-syntax-comment: #6a737d !important;
                            --color-prettylights-syntax-constant: #79c0ff !important;
                            --color-prettylights-syntax-entity: #d2a8ff !important;
                            --color-prettylights-syntax-storage-modifier-import: #c9d1d9 !important;
                            --color-prettylights-syntax-entity-tag: #7ee787 !important;
                            --color-prettylights-syntax-keyword: #ff7b72 !important;
                            --color-prettylights-syntax-string: #a5d6ff !important;
                            --color-prettylights-syntax-variable: #ffa657 !important;
                            --color-prettylights-syntax-brackethighlighter-unmatched: #f85149 !important;
                            --color-prettylights-syntax-invalid-illegal-text: #f0f6fc !important;
                            --color-prettylights-syntax-invalid-illegal-bg: #da3633 !important;
                            --color-prettylights-syntax-carriage-return-text: #f0f6fc !important;
                            --color-prettylights-syntax-carriage-return-bg: #ff7b72 !important;
                            --color-prettylights-syntax-string-regexp: #7ee787 !important;
                            --color-prettylights-syntax-markup-list: #e3b341 !important;
                            --color-prettylights-syntax-markup-heading: #1f6feb !important;
                            --color-prettylights-syntax-markup-italic: #c9d1d9 !important;
                            --color-prettylights-syntax-markup-bold: #c9d1d9 !important;
                            --color-prettylights-syntax-markup-deleted-text: #ffdcd7 !important;
                            --color-prettylights-syntax-markup-deleted-bg: #67060c !important;
                            --color-prettylights-syntax-markup-inserted-text: #aff5b4 !important;
                            --color-prettylights-syntax-markup-inserted-bg: #033a16 !important;
                            --color-prettylights-syntax-markup-changed-text: #ffd8a8 !important;
                            --color-prettylights-syntax-markup-changed-bg: #5a1e02 !important;
                            --color-prettylights-syntax-markup-ignored-text: #c9d1d9 !important;
                            --color-prettylights-syntax-markup-ignored-bg: #1e1e1e !important;
                            --color-prettylights-syntax-meta-diff-range: #d2a8ff !important;
                            --color-prettylights-syntax-brackethighlighter-angle: #8b949e !important;
                            --color-prettylights-syntax-sublimelinter-gutter-mark: #484f58 !important;
                            --color-prettylights-syntax-constant-other-reference-link: #a5d6ff !important;

                            --color-fg-default: #d4d4d4 !important;
                            --color-fg-muted: #a0a0a0 !important;
                            --color-fg-subtle: #888888 !important;
                            --color-canvas-default: #1e1e1e !important;
                            --color-canvas-subtle: #252526 !important;
                            --color-border-default: #3e3e42 !important;
                            --color-border-muted: rgba(110, 118, 129, 0.4) !important;
                            --color-neutral-muted: rgba(110, 118, 129, 0.1) !important;
                            --color-accent-fg: #569cd6 !important;
                            --color-accent-emphasis: #4e94d4 !important;
                            --color-attention-subtle: #5c5c5c !important;
                            --color-danger-fg: #f85149 !important;

                            /* General settings */
                            color: var(--color-fg-default) !important;
                            background-color: var(--color-canvas-default) !important;
                            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji" !important;
                            font-size: 16px !important;
                            line-height: 1.5 !important;
                            word-wrap: break-word !important;
                        }
                        body {
                            font-family: Arial, sans-serif;
                            background-color: #121212;
                            color: #e0e0e0;
                        }
                         .container {
                            max-width: 1300px;
                            margin: 0 auto;
                        }
                        .header {
                            background-image: url('https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Fpreview.redd.it%2Fi0h9ke187tk31.png%3Fwidth%3D960%26crop%3Dsmart%26auto%3Dwebp%26s%3Ddc294c8327d576f78d3cd0e08982cd6e3f619a21&f=1&nofb=1&ipt=47a8aff3e3499390c872b22b77ba3ad02b9f28fc0c0f5b5d3d82c84dd16ed6a6&ipo=images');
                            background-position: center;
                            color: #f0f0f0;
                            padding: 20px;
                            text-align: center;
                            background-size: cover;
                        }
                        .post-card {
                            background-color: #1e1e1e;
                            color: #e0e0e0;
                            border: none;
                            margin-bottom: 20px;
                            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                            transition: 0.3s;
                        }
                        .post-card:hover {
                            box-shadow: 0 8px 16px rgba(0, 0, 0, 0.5);
                        }
                        .sidebar {
                            background-color: #242424;
                            color: #e0e0e0;
                            padding: 20px;
                            border-radius: 8px;
                        }
                        .footer {
                            background-color: #1c1c1c;
                            color: #f0f0f0;
                            text-align: center;
                            padding: 15px;
                            margin-top: 20px;
                        }
                        .navbar-nav .nav-link {
                            color: #e0e0e0 !important;
                        }
                        .btn-primary {
                            background-color: #007bff;
                            border-color: #007bff;
                        }
                        .btn-outline-primary {
                            color: #007bff;
                            border-color: #007bff;
                        }
                        .btn-outline-primary:hover {
                            background-color: #007bff;
                            color: #fff;
                        }
                        .post-body {
                            background-color: #1e1e1e;
                            padding: 20px;
                            border-radius: 8px;
                            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                        }
                "# }
                }
                body{
                    div class="header" {
                        h1 { "Res techinca fortuitae" }
                    }
                    a href="/" class="btn mt-4" { "< Back" }
                    // Main Content Container
                    div class="container" {
                        h2 { (post.title) }
                        p class="text-muted" { (post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()) }
                        div class="post-body" {
                            github-md {
                                (&post.body)
                            }
                        }
                    }

                    // Footer
                    div class="footer" {
                        p { "&copy; 2024 Res techinca fortuitae | Designed by Caden Ream" }
                    }
                }
            }
        };
        Html(rendered_html.into_string())
    }   else {
        // Render a 404 page with consistent styling if the post is not found
        let rendered_html = html! {
            (maud::DOCTYPE)
            html lang="en" {
                head {
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    title { "404 - Post Not Found" }
                    link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                    style { r#"
                    body {
                        font-family: Arial, sans-serif;
                        background-color: #121212;
                        color: #e0e0e0;
                    }
                    .header {
                        background-image: url('https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Fpreview.redd.it%2Fi0h9ke187tk31.png%3Fwidth%3D960%26crop%3Dsmart%26auto%3Dwebp%26s%3Ddc294c8327d576f78d3cd0e08982cd6e3f619a21&f=1&nofb=1&ipt=47a8aff3e3499390c872b22b77ba3ad02b9f28fc0c0f5b5d3d82c84dd16ed6a6&ipo=images');
                        background-position: center;
                        color: #f0f0f0;
                        padding: 20px;
                        text-align: center;
                        background-size: cover;
                    }
                    .post-card {
                        background-color: #1e1e1e;
                        color: #e0e0e0;
                        border: none;
                        margin-bottom: 20px;
                        box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                        transition: 0.3s;
                    }
                    .post-card:hover {
                        box-shadow: 0 8px 16px rgba(0, 0, 0, 0.5);
                    }
                    .sidebar {
                        background-color: #242424;
                        color: #e0e0e0;
                        padding: 20px;
                        border-radius: 8px;
                    }
                    .footer {
                        background-color: #1c1c1c;
                        color: #f0f0f0;
                        text-align: center;
                        padding: 15px;
                        margin-top: 20px;
                    }
                    .navbar-nav .nav-link {
                        color: #e0e0e0 !important;
                    }
                    .btn-primary {
                        background-color: #007bff;
                        border-color: #007bff;
                    }
                    .btn-outline-primary {
                        color: #007bff;
                        border-color: #007bff;
                    }
                    .btn-outline-primary:hover {
                        background-color: #007bff;
                        color: #fff;
                    }
                "# }
                }
                body {
                    // Header
                    div class="header" {
                        h1 { "Res techinca fortuitae" }
                    }

                    // Main Content Container
                    div class="container" {
                        div class="error-message" {
                            h2 { "404 - Post Not Found" }
                            p { "The post Caden Ream are looking for does not exist." }
                            a href="/" class="btn btn-primary mt-4" { "Back to Home" }
                        }
                    }

                    // Footer
                    div class="footer" {
                        p { "©2024 Res techinca fortuitae | Designed by Caden Ream" }
                    }
                }
            }
        };
        Html(rendered_html.into_string())
    }

}

#[tokio::test]
async fn test() {
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    let app = Router::new().route("/", get(home));
    let response = app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), 1024000).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "html");
//    assert!(body_str.contains("Test content"));
}
