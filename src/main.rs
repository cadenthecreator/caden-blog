use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::Router;
use axum::routing::get;
use chrono::{DateTime, Local, Utc};
use chrono_tz::Tz;
use maud::{html, Markup, PreEscaped, DOCTYPE};
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

    if !path.is_dir() {
        println!("Directory {} does not exist.", dir);
        return vec![];
    }

    let mut file_list = Vec::new();
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
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

fn render_post(post: &Post) -> Markup {
    html! {
        div class="post" {
            h1 { (post.title) }
            p class="text-muted" { (post.timestamp.format("%b %-d, %Y %-I:%M %p %Z").to_string()) }
            div class="post-content" {
                (markdown_to_html(&post.body,&Options::default()))
            }
        }
    }
}

async fn load_file(filename: &str, cache: FileCache) -> Option<Vec<u8>> {
    let filepath = format!("./caden-blog/assets/{}", filename);
    let mut file = File::open(&filepath).ok()?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).ok()?;

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
    match String::from_utf8(content.clone()) {
        Err(..) => Response::builder()
            .header(CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000"))
            .body(Body::from(content))
            .unwrap(),
        Ok(str) => Response::builder()
            .header(CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000"))
            .body(Body::from(str))
            .unwrap(),
    }
}

async fn handle_asset_request(Path(filename): Path<String>, cache: FileCache) -> Result<Response<Body>, StatusCode> {
    if let Some(content) = cache.lock().expect("cdn failed to lock the cache").get(&filename).cloned() {
        return Ok(cache_control_response(content));
    }

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

    let mut file = File::open(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    let mut contents = Vec::new();

    file.read_to_end(&mut contents).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    if path.exists() && !display.to_string().contains("..") {
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


fn render_posts_fragment<'a, I>(posts: I, tz: Tz, tag: Option<&str>) -> String
where
    I: IntoIterator<Item = &'a Post>,
{
    // returns ONLY the fragment container that matches your page’s target
    println!("{}",Local::now());
    html! {
        @for post in posts {
            @if post.timestamp < Utc::now() && (tag.is_none() || post.tags.contains(&tag.unwrap_or_default().to_string())) {
                div class="card" {
                    img src=(post.image_url) class="post-image" alt="Post Image";
                    div class="post-body" {
                        h2 { (post.title) }
                        p class="timestamp" {
                            (format!(
                                "Posted on {}",
                                post.timestamp.with_timezone(&tz)
                                .format("%b %-d, %Y %-I:%M %p %Z")))
                        }
                        p class="summary" { (post.summary) }
                        a href=(format!("/post/{}", post.url_name)) class="btn" { "Read More" }
                    }
                }
            }
        }
    }.into_string()
}

async fn posts(
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    UserTz(tz): UserTz,
) -> impl IntoResponse {
    // Load posts (same as before)
    let mut all_posts: Vec<Post> = vec![];
    for f in list_files_in_directory("./caden-blog/posts") {
        if let Some(p) = get_from_file(&f) { all_posts.push(p); }
    }
    let tag = params.get("tag").map(String::as_str);
    let filtered: Vec<&Post> = match tag {
        Some(t) => all_posts.iter().filter(|p| p.tags.iter().any(|x| x == t)).collect(),
        None => all_posts.iter().collect(),
    };

    // Build fragment body
    let body = render_posts_fragment(filtered, tz, tag);

    // Always 200 OK, text/html
    Html(body).into_response()
}

async fn home(UserTz(tz): UserTz) -> Html<String> {
    let mut posts: Vec<Post> = vec![];
    for file in list_files_in_directory("./caden-blog/posts") {
        posts.push(get_from_file(&file).unwrap());
    }
    Html(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                script src="/assets/htmx.min.js" integrity="sha384-ZBXiYtYQ6hJ2Y0ZNoYuI+Nq5MqWBr+chMrS/RkXpNzQCApHEhOt2aY8EJgqwHLkJ" crossorigin="anonymous" {}
                script {
                    (PreEscaped(r#"
                    (function setTzCookie(){
                      try {
                        var tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
                        // only (re)write when missing/changed
                        if (!document.cookie.includes('tz=' + encodeURIComponent(tz))) {
                          document.cookie = 'tz=' + encodeURIComponent(tz) + '; Path=/; Max-Age=31536000; SameSite=Lax';
                        }
                      } catch (e) {}
                    })();
                    "#))
                }

                script {
                    (PreEscaped(r#"
                    document.addEventListener('htmx:configRequest', function (e) {
                      try {
                        e.detail.headers['X-Time-Zone'] =
                          Intl.DateTimeFormat().resolvedOptions().timeZone;
                      } catch (e) {}
                    });
                    "#))
                }
                title { "Phase Space" }
                style { (PreEscaped(r#"

@-webkit-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@-moz-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}


:root { --speed: 0.5; }

html {
background: repeating-linear-gradient( -45deg, #00000000, #00000000 20px, #00000053 20px, #00000053 40px );
background-size: 400% 400%;
    background-color: rgb(13, 39, 75);
    background-repeat: repeat;
    background-position: 0 0;
    background-attachment: fixed;
    font-family: Arial, sans-serif;

    /* Use a duration that shrinks as --speed grows */
    animation-name: scroll;
    animation-duration: calc(300s / var(--speed));
    animation-timing-function: linear;
    animation-iteration-count: infinite;

    /* Optional vendor shorthands if you need them */
    -webkit-animation-name: scroll;
    -webkit-animation-duration: calc(300s / var(--speed));
    -webkit-animation-timing-function: linear;
    -webkit-animation-iteration-count: infinite;

}
.title {
    font-family: Arial, sans-serif;
    text-align: center;
    font-size: 700%;
    color: rgb(191, 191, 191);
    mix-blend-mode:color-dodge;
    margin: 1%;
}
.background {
    background-color: rgb(25, 49, 85);
    color: white;
    margin-left: 5%;
    margin-right: 5%;
    padding: 3rem;
}
.row {
    display: flex;
}
.sidebar {
    background-color: rgb(49, 60, 98);
    padding: 20px;
    padding-top: 0px;
    height: fit-content;
    padding-bottom: 0px;
    width: 20rem;
}
.content {
    display: grid;
    grid-template-columns: repeat(auto-fill, 400px);
    gap: 1rem;
    width: 100%;
}
.card {
    background-color: rgb(49, 60, 98);
    padding: 5px;
    height: 600px;
    width: 400px;
    margin: 1rem;
}
.post-image {
    width: 390px;
    margin-left: auto;
    margin-right: auto;
}
.btn {
    color: rgb(120, 158, 240);
    text-decoration: none;
    background-color: rgb(43, 43, 70);
    border-color: rgb(67, 71, 96);
    border-width: 5px;
    margin-left: auto;
    margin-right: auto;
    display: block;
    width: 5rem ;
    border-style: solid;
}

.btn:hover {
    color: rgb(209, 223, 255);
}

.btn-tag{

  display: inline-flex;
  align-items: center;
  gap: .4em;
  color: inherit;
  font-size: 1.5rem;
  text-decoration: none;
  background: transparent;
  line-height: 1;
  border-width: 0px;
}

.btn-tag:hover{ opacity: .85; }

.btn-tag:focus-visible{
  outline: 2px solid currentColor;
  outline-offset: 2px;
}
"#)) }
            }
            body {
                h1 class="title" { "Phase Space" }
                div class="background" {
                    div class="row" {
                        // Maud turns underscores into hyphens in attribute names:
                        div class="content" hx-get="/posts" hx-trigger="load" {}
                        div class="sidebar" id="sidebar" {
                            h2 { "Info" }
                            p { "Welcome to my blog! I am primarily doing this for school.. but will likely continue posting outside of school. I enjoy talking about tech stuff mainly software. But I also enjoy talking about robotics, hardware and science. I hope you find this useful or entertaining somewhat. :)" }
                            hr;
                            ul {
                                li { button class = "btn-tag" hx-vals=r#"{"tag": "robotics"}"# hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "Robotics" } }
                                li { button class = "btn-tag" hx-vals=r#"{"tag": "hardware"}"# hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "Hardware" } }
                                li { button class = "btn-tag" hx-vals=r#"{"tag": "software"}"# hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "Software" } }
                                li { button class = "btn-tag" hx-vals=r#"{"tag": "gaming"}"# hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "Gaming" } }
                                li { button class = "btn-tag" hx-vals=r#"{"tag": "science"}"# hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "Science" } }
                                li { button class = "btn-tag" hx-get="/posts" hx-target=".content" hx-swap="innerHTML" { "All" } }
                            }
                        }
                    }
                }
            }
        }
    }.into_string())
}

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::CookieJar;
use comrak::{markdown_to_html, Options};

pub struct UserTz(pub Tz);

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for UserTz {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S)
                                -> Result<Self, Self::Rejection>
    {
        if let Some(tz) = parts.headers
            .get("X-Time-Zone")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<Tz>().ok())
        {
            return Ok(UserTz(tz));
        }

        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(tz) = jar.get("tz")
            .and_then(|c| c.value().parse::<Tz>().ok())
        {
            return Ok(UserTz(tz));
        }

        Ok(UserTz(chrono_tz::UTC))
    }
}


async fn post_handler(Path(url_name): Path<String>, headers: HeaderMap, UserTz(tz): UserTz) -> Html<String> {
    let dir = format!("./caden-blog/posts/{}.json",url_name);
    let path = std::path::Path::new((&dir).into());
    let display = path.display();
    if path.exists() && !display.to_string().contains("..") {
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
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                // GitHub dark (auto when prefers-color-scheme: dark)
                link rel="stylesheet"
                     href="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/styles/github-dark-dimmed.min.css";

                // highlight.js core (common languages)
                script src="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/highlight.min.js" {}

                // (Optional) ensure Rust grammar is present
                script src="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/languages/rust.min.js" {}

                // init after scripts load
                script { (PreEscaped("hljs.highlightAll();")) }

                // optional: GitHub-like box spacing for blocks
                style { (PreEscaped(r#"
                    pre { padding: 16px; border-radius: 6px; overflow: auto; }
                    pre > code { background: transparent; padding: 0; }
                "#)) }
                script src="/assets/htmx.min.js" integrity="sha384-ZBXiYtYQ6hJ2Y0ZNoYuI+Nq5MqWBr+chMrS/RkXpNzQCApHEhOt2aY8EJgqwHLkJ" crossorigin="anonymous" {}
                script {
                    (PreEscaped(r#"
                    (function setTzCookie(){
                      try {
                        var tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
                        // only (re)write when missing/changed
                        if (!document.cookie.includes('tz=' + encodeURIComponent(tz))) {
                          document.cookie = 'tz=' + encodeURIComponent(tz) + '; Path=/; Max-Age=31536000; SameSite=Lax';
                        }
                      } catch (e) {}
                    })();
                    "#))
                }

                script {
                    (PreEscaped(r#"
                    document.addEventListener('htmx:configRequest', function (e) {
                      try {
                        e.detail.headers['X-Time-Zone'] =
                          Intl.DateTimeFormat().resolvedOptions().timeZone;
                      } catch (e) {}
                    });
                    "#))
                }
                title { (format!("Phase Space - {}",&post.title)) }
                style { (PreEscaped(r#"

@-webkit-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@-moz-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}


:root { --speed: 0.5; }

html {
background: repeating-linear-gradient( -45deg, #00000000, #00000000 20px, #00000053 20px, #00000053 40px );
background-size: 400% 400%;
    background-color: rgb(13, 39, 75);
    background-repeat: repeat;
    background-position: 0 0;
    background-attachment: fixed;
    font-family: Arial, sans-serif;

    /* Use a duration that shrinks as --speed grows */
    animation-name: scroll;
    animation-duration: calc(300s / var(--speed));
    animation-timing-function: linear;
    animation-iteration-count: infinite;

    /* Optional vendor shorthands if you need them */
    -webkit-animation-name: scroll;
    -webkit-animation-duration: calc(300s / var(--speed));
    -webkit-animation-timing-function: linear;
    -webkit-animation-iteration-count: infinite;

}
.title {
    font-family: Arial, sans-serif;
    text-align: center;
    font-size: 700%;
    color: rgb(191, 191, 191);
    mix-blend-mode:color-dodge;
    margin: 1%;
}
.background {
    display: block;
    background-color: rgb(25, 49, 85);
    color: white;
    margin: 0 auto;
    width: 70%;
    overflow-wrap: anywhere;
    padding: 3rem;
}
.row {
    display: flex;
}
.content {
    width: 100%;
}
.card {
    background-color: rgb(49, 60, 98);
    outline-width: 5px;
    outline-color:rgb(104, 132, 156);
    outline-style:solid;
    padding-bottom: 5px;
    height: fit-content;
    width: 500px;
    margin: 1rem;
}
.card-img-top {
    width: 500px;
}
.btn {
    color: rgb(120, 158, 240);
    text-decoration: none;
    background-color: rgb(43, 43, 70);
    border-color: rgb(67, 71, 96);
    border-width: 5px;
    margin-left: auto;
    margin-right: auto;
    display: block;
    width: 5rem ;
    border-style: solid;
}

.btn:hover {
    color: rgb(209, 223, 255);
}
a {
    color: rgb(135, 246, 255)
}

code.hljs {
    background: rgb(15, 29, 65);
}

.back-btn{

  display: inline-flex;
  align-items: center;
  gap: .4em;
  color: rgb(200,200,200);
  font-size: 1.5rem;
  text-decoration: none;
  background: transparent;
  line-height: 1;
}

.back-btn:hover{ opacity: .85; }

.back-btn:focus-visible{
  outline: 2px solid currentColor;
  outline-offset: 2px;
}
"#)) }
            }
            body {
                h1 class="title" { (&post.title) }
                div class="background" {
                    div class="content" {
                            a href="/" class="back-btn" {"< Back"}
                        (PreEscaped(markdown_to_html(&post.body,&Options::default())))
                    }
                }
            }
        }
    };
        Html(rendered_html.into_string())
    }   else {
        let rendered_html = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                // GitHub dark (auto when prefers-color-scheme: dark)
                link rel="stylesheet"
                     href="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/styles/github-dark-dimmed.min.css";

                // highlight.js core (common languages)
                script src="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/highlight.min.js" {}

                // (Optional) ensure Rust grammar is present
                script src="https://unpkg.com/@highlightjs/cdn-assets@11.9.0/languages/rust.min.js" {}

                // init after scripts load
                script { (PreEscaped("hljs.highlightAll();")) }

                // optional: GitHub-like box spacing for blocks
                style { (PreEscaped(r#"
                    pre { padding: 16px; border-radius: 6px; overflow: auto; }
                    pre > code { background: transparent; padding: 0; }
                "#)) }
                script src="/assets/htmx.min.js" integrity="sha384-ZBXiYtYQ6hJ2Y0ZNoYuI+Nq5MqWBr+chMrS/RkXpNzQCApHEhOt2aY8EJgqwHLkJ" crossorigin="anonymous" {}
                script {
                    (PreEscaped(r#"
                    (function setTzCookie(){
                      try {
                        var tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
                        // only (re)write when missing/changed
                        if (!document.cookie.includes('tz=' + encodeURIComponent(tz))) {
                          document.cookie = 'tz=' + encodeURIComponent(tz) + '; Path=/; Max-Age=31536000; SameSite=Lax';
                        }
                      } catch (e) {}
                    })();
                    "#))
                }

                script {
                    (PreEscaped(r#"
                    document.addEventListener('htmx:configRequest', function (e) {
                      try {
                        e.detail.headers['X-Time-Zone'] =
                          Intl.DateTimeFormat().resolvedOptions().timeZone;
                      } catch (e) {}
                    });
                    "#))
                }
                title { "Phase Space - 404 not found" }
                style { (PreEscaped(r#"

@-webkit-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@-moz-keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}
@keyframes scroll {
    0% { background-position:50% 0% }
    50% { background-position:51% 100% }
    100% { background-position:50% 0% }
}


:root { --speed: 0.5; }

html {
background: repeating-linear-gradient( -45deg, #00000000, #00000000 20px, #00000053 20px, #00000053 40px );
background-size: 400% 400%;
    background-color: rgb(13, 39, 75);
    background-repeat: repeat;
    background-position: 0 0;
    background-attachment: fixed;
    font-family: Arial, sans-serif;

    /* Use a duration that shrinks as --speed grows */
    animation-name: scroll;
    animation-duration: calc(300s / var(--speed));
    animation-timing-function: linear;
    animation-iteration-count: infinite;

    /* Optional vendor shorthands if you need them */
    -webkit-animation-name: scroll;
    -webkit-animation-duration: calc(300s / var(--speed));
    -webkit-animation-timing-function: linear;
    -webkit-animation-iteration-count: infinite;

}
.title {
    font-family: Arial, sans-serif;
    text-align: center;
    font-size: 700%;
    color: rgb(191, 191, 191);
    mix-blend-mode:color-dodge;
    margin: 1%;
    text-decoration: none;
}
.background {
    background-color: rgb(25, 49, 85);
    color: white;
    margin-left: 20%;
    margin-right: 20%;
    padding: 3rem;
}
.row {
    display: flex;
}
.content {
    width: 100%;
}
.card {
    background-color: rgb(49, 60, 98);
    outline-width: 5px;
    outline-color:rgb(104, 132, 156);
    outline-style:solid;
    padding-bottom: 5px;
    height: fit-content;
    width: 500px;
    margin: 1rem;
}
.card-img-top {
    width: 500px;
}
.btn {
    color: rgb(120, 158, 240);
    text-decoration: none;
    background-color: rgb(43, 43, 70);
    border-color: rgb(67, 71, 96);
    border-width: 5px;
    margin-left: auto;
    margin-right: auto;
    display: block;
    width: 5rem ;
    border-style: solid;
}

.btn:hover {
    color: rgb(209, 223, 255);
}
a {
    color: rgb(135, 246, 255)
}

code.hljs {
    background: rgb(15, 29, 65);
}
.back-btn{

  display: inline-flex;
  align-items: center;
  gap: .4em;
  color: inherit;
  font-size: 2rem;
  text-decoration: none;
  background: transparent;
  line-height: 1;
}

.back-btn:hover{ opacity: .85; }

.back-btn:focus-visible{
  outline: 2px solid currentColor;
  outline-offset: 2px;
}
"#)) }
            }
            body {
                h1 class="title" { "Are you lost?" }
                div class="background" {
                    div class="content" {
                        h1 {"This page doesn't exist.."} h1 {"sorry to disappoint."} a href="/" class = "back-btn" {"> Go Home <"}
                    }
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
}
