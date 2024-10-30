use axum::response::Html;
use axum::Router;
use axum::routing::get;
use maud::{html, DOCTYPE};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening to {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<String> {
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
                    }
                    .header {
                        background-color: #343a40;
                        color: white;
                        padding: 20px;
                        text-align: center;
                    }
                    .navbar-nav .nav-link {
                        color: white !important;
                    }
                    .post-card {
                        margin-bottom: 20px;
                        box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
                        transition: 0.3s;
                    }
                    .post-card:hover {
                        box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
                    }
                    .sidebar {
                        padding: 20px;
                        background-color: #f8f9fa;
                        border-radius: 8px;
                    }
                    .footer {
                        background-color: #343a40;
                        color: white;
                        text-align: center;
                        padding: 15px;
                        margin-top: 20px;
                    }
                "# }
            }
            body {
                // Header
                div class="header" {
                    h1 { "My Fancy Blog" }
                    p { "Your daily dose of awesome reads!" }
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
                            // Post Card 1
                            div class="card post-card" {
                                img src="https://via.placeholder.com/800x400" class="card-img-top" alt="Post Image";
                                div class="card-body" {
                                    h5 class="card-title" { "Amazing Blog Post Title" }
                                    p class="text-muted" { "Posted on October 29, 2024 by " strong { "John Doe" } }
                                    p class="card-text" { "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Quisque nec nisi tortor. Aenean vitae lacus vel est malesuada..." }
                                    a class="btn btn-primary" up-target=".modal-content" up-layer="new" up-content="Test" { "Read More" }
                                }
                            }

                            // Post Card 2
                            div class="card post-card" {
                                img src="https://via.placeholder.com/800x400" class="card-img-top" alt="Post Image";
                                div class="card-body" {
                                    h5 class="card-title" { "Another Exciting Post" }
                                    p class="text-muted" { "Posted on October 28, 2024 by " strong { "Jane Smith" } }
                                    p class="card-text" { "Proin euismod mauris vel nibh convallis, at pharetra magna posuere. Morbi gravida justo ac sapien dictum vestibulum..." }
                                    a class="btn btn-primary" up-target=".modal-content" up-layer="new" up-content="Test" { "Read More" }
                                }
                            }
                        }

                        // Sidebar
                        div class="col-lg-4" {
                            div class="sidebar" {
                                h4 { "About Me" }
                                p { "I'm a passionate writer sharing insights on various topics. Welcome to my blog!" }
                                hr;
                                h5 { "Categories" }
                                ul class="list-unstyled" {
                                    li { a href="#" { "Tech" } }
                                    li { a href="#" { "Lifestyle" } }
                                    li { a href="#" { "Travel" } }
                                    li { a href="#" { "Food" } }
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
                    p { "&copy; 2024 Fancy Blog | Designed by You" }
                }

                // Bootstrap JavaScript Bundle
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
                script src="https://unpkg.com/unpoly@4.0.0/dist/unpoly.min.js" {}
            }
        }
    }.into_string())
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
