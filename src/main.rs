use std::fs;
use std::fs::File;
use std::io::Read;
use axum::extract::Path;
use axum::response::Html;
use axum::Router;
use axum::routing::get;
use chrono::{DateTime, Utc};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
    title: String,
    body: String,
    image_url: String,
    summary: String,
    timestamp: DateTime<Utc>,
    #[serde(skip)]
    url_name: String,
}

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

fn serialize_post(post: &Post) -> String {
    serde_json::to_string(post).expect("Failed to serialize Post")
}

fn deserialize_post(json_data: &str,url_name: &str) -> Post {
    let mut post: Post = serde_json::from_str(json_data).expect("Failed to deserialize Post");
    post.url_name = url_name.to_string();
    post
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(handler)).route("/post/:url_name", get(post_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening to {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
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

async fn handler() -> Html<String> {
    let mut posts: Vec<Post> = vec![];
    for file in list_files_in_directory("./caden-blog/posts") {
        posts.push(get_from_file(&file).unwrap());
        //println!("{}", file);
    }
    // for post in &posts {
    //     println!("{}", serialize_post(&post));
    // }
    Html(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Fancy Blog" }
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                link rel="stylesheet" href="https://unpkg.com/unpoly@4.0.0/dist/unpoly.min.css";
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
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
                script src="https://unpkg.com/unpoly@4.0.0/dist/unpoly.min.js" {}
                // Header
                div class="header" {
                    h1 { "The Caden Times" }
                    p { "I don't know why you are here" }
                }

                // Navigation Bar
                nav class="navbar navbar-expand-lg navbar-dark bg-dark" {
                    div class="container" {
                        a class="navbar-brand" href="#" { "Fancy Blog" }
                        button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarNav" aria-controls="navbarNav" aria-expanded="false" aria-label="Toggle navigation" {
                            span class="navbar-toggler-icon" {}
                        }
                        div class="collapse navbar-collapse" id="navbarNav" {
                            ul class="navbar-nav ms-auto" {
                                li class="nav-item" {
                                    a class="nav-link active" href="#" { "Home" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="#" { "About" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="#" { "Contact" }
                                }
                            }
                        }
                    }
                }

                // Main Content
                div class="container my-4" {
                    div class="row" {
                        // Blog Posts
                        div class="col-lg-8" {
                            @for post in posts {
                                div class="card post-card" {
                                    img src=(post.image_url) class="card-img-top" alt="Post Image";
                                    div class="card-body" {
                                        h5 class="card-title" { (post.title) }
                                        p class="text-muted" { (format!("Posted on {}", post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()))}
                                        p class="card-text" { (post.summary) }
                                        a href=(format!("/post/{}",post.url_name)) class="btn btn-primary" up-target=".modal-content" up-layer="new" { "Read More" }
                                    }
                                }
                            }
                        }

                        // Sidebar
                        div class="col-lg-4" {
                            div class="sidebar" {
                                h4 { "About Me" }
                                p { "I'm an unmotivated nerd that is making this for absolutely no reason." }
                                hr;
                                h5 { "Categories" }
                                ul class="list-unstyled" {
                                    li { a href="#" { "Tech" } }
                                    li { a href="#" { "Programming" } }
                                    li { a href="#" { "Computer Science" } }
                                    li { a href="#" { "Software Engineering" } }
                                }
                                hr;
                                h5 { "Follow Me" }
                                a href="#" class="btn btn-outline-primary btn-sm" { "Twitter" }
                                a href="#" class="btn btn-outline-primary btn-sm" { "Facebook" }
                                a href="#" class="btn btn-outline-primary btn-sm" { "Instagram" }
                            }
                        }
                    }
                }

                // Footer
                div class="footer" {
                    p { "©2024 The Caden Times | Designed by CadenTheCreator" }
                }

                // Bootstrap JavaScript Bundle
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
            html lang="en" {
                head {
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    title { (post.title) }
                    link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                    style { r#"
                        body {
                            font-family: Arial, sans-serif;
                            background-color: #121212;
                            color: #e0e0e0;
                            padding: 20px;
                        }
                        .container {
                            max-width: 800px;
                            margin: 0 auto;
                        }
                        .header, .footer {
                            text-align: center;
                            background-color: #343a40;
                            color: #f0f0f0;
                            padding: 20px;
                        }
                        .post-body {
                            background-color: #1e1e1e;
                            padding: 20px;
                            border-radius: 8px;
                            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                        }
                        .footer {
                            margin-top: 20px;
                        }
                        .btn-primary {
                            background-color: #007bff;
                            border-color: #007bff;
                        }
                    "# }
                }
                body {
                    // Header
                    div class="header" {
                        h1 { "The Caden Times" }
                    }

                    // Main Content Container
                    div class="container" {
                        h2 { (post.title) }
                        p class="text-muted" { (post.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()) }
                        div class="post-body" {
                            (markdown_to_html(&post.body))
                        }
                        a href="/" class="btn btn-primary mt-4" { "Back to Home" }
                    }

                    // Footer
                    div class="footer" {
                        p { "&copy; 2024 Fancy Blog | Designed by You" }
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
                            padding: 20px;
                        }
                        .container {
                            max-width: 800px;
                            margin: 0 auto;
                            text-align: center;
                        }
                        .header, .footer {
                            text-align: center;
                            background-color: #343a40;
                            color: #f0f0f0;
                            padding: 20px;
                        }
                        .error-message {
                            background-color: #1e1e1e;
                            padding: 20px;
                            border-radius: 8px;
                            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                        }
                        .footer {
                            margin-top: 20px;
                        }
                        .btn-primary {
                            background-color: #007bff;
                            border-color: #007bff;
                        }
                    "# }
                }
                body {
                    // Header
                    div class="header" {
                        h1 { "The Caden Times" }
                    }

                    // Main Content Container
                    div class="container" {
                        div class="error-message" {
                            h2 { "404 - Post Not Found" }
                            p { "The post you are looking for does not exist." }
                            a href="/" class="btn btn-primary mt-4" { "Back to Home" }
                        }
                    }

                    // Footer
                    div class="footer" {
                        p { "&copy; 2024 Fancy Blog | Designed by You" }
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

    let app = Router::new().route("/", get(handler));
    let response = app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), 1024000).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "html");
//    assert!(body_str.contains("Test content"));
}
