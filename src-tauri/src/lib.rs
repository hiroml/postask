use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct User {
    id: String,
    username: String,
    password: String,
    name: String,
    bio: String,
    followers: usize,
    following: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Post {
    id: String,
    author_id: String,
    author_name: String,
    handle: String,
    content: String,
    created_at: String,
    likes: usize,
    retweets: usize,
    replies: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ProfileData {
    user: User,
    posts: Vec<Post>,
}

fn db_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".x_clone.sqlite")
}

fn now_stamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    now.to_string()
}

fn open_db() -> SqliteResult<Connection> {
    let path = db_path();
    let conn = Connection::open(path)?;
    init_db(&conn)?;
    Ok(conn)
}

fn init_db(conn: &Connection) -> SqliteResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL,
            name TEXT NOT NULL,
            bio TEXT NOT NULL DEFAULT '',
            followers INTEGER NOT NULL DEFAULT 0,
            following INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS posts (
            id TEXT PRIMARY KEY,
            author_id TEXT NOT NULL,
            author_name TEXT NOT NULL,
            handle TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            likes INTEGER NOT NULL DEFAULT 0,
            retweets INTEGER NOT NULL DEFAULT 0,
            replies INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS likes (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            post_id TEXT NOT NULL,
            UNIQUE(user_id, post_id)
        );

        CREATE TABLE IF NOT EXISTS retweets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            post_id TEXT NOT NULL,
            UNIQUE(user_id, post_id)
        );
        "#,
    )?;

    seed_demo_state(conn)?;
    Ok(())
}

fn seed_demo_state(conn: &Connection) -> SqliteResult<()> {
    let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
    if user_count > 0 {
        return Ok(());
    }

    let demo_users = vec![
        (
            "u_demo_1",
            "aiko",
            "password",
            "Aiko Tanaka",
            "プロダクトと人をつなぐのが好きです。",
            1280,
            420,
        ),
        (
            "u_demo_2",
            "yuki",
            "password",
            "Yuki Sato",
            "毎日少しずつ改善しています。",
            940,
            220,
        ),
    ];

    for (id, username, password, name, bio, followers, following) in demo_users {
        conn.execute(
            "INSERT INTO users (id, username, password, name, bio, followers, following, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, username, password, name, bio, followers, following, now_stamp()],
        )?;
    }

    let demo_posts = vec![
        (
            "p_demo_1",
            "u_demo_1",
            "Aiko Tanaka",
            "@aiko",
            "新しいプロジェクトを始めたので、小さく始めて改善を続けていきます。",
            182,
            26,
            14,
        ),
        (
            "p_demo_2",
            "u_demo_2",
            "Yuki Sato",
            "@yuki",
            "今日は集中できたので、少しずつでも前に進めています。",
            91,
            12,
            7,
        ),
    ];

    for (id, author_id, author_name, handle, content, likes, retweets, replies) in demo_posts {
        conn.execute(
            "INSERT INTO posts (id, author_id, author_name, handle, content, created_at, likes, retweets, replies) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, author_id, author_name, handle, content, now_stamp(), likes, retweets, replies],
        )?;
    }

    Ok(())
}

fn user_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password: row.get(2)?,
        name: row.get(3)?,
        bio: row.get(4)?,
        followers: row.get(5)?,
        following: row.get(6)?,
    })
}

fn post_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Post> {
    Ok(Post {
        id: row.get(0)?,
        author_id: row.get(1)?,
        author_name: row.get(2)?,
        handle: row.get(3)?,
        content: row.get(4)?,
        created_at: row.get(5)?,
        likes: row.get(6)?,
        retweets: row.get(7)?,
        replies: row.get(8)?,
    })
}

#[tauri::command]
fn login(username: String, password: String) -> Result<User, String> {
    let username = username.trim().to_string();
    let password = password.trim().to_string();

    if username.is_empty() || password.is_empty() {
        return Err("ユーザー名とパスワードを入力してください".to_string());
    }

    let conn = open_db().map_err(|e| e.to_string())?;

    let existing = conn
        .query_row(
            "SELECT id, username, password, name, bio, followers, following FROM users WHERE username = ?1",
            [&username],
            user_from_row,
        );

    match existing {
        Ok(user) => {
            if user.password != password {
                return Err("パスワードが正しくありません".to_string());
            }
            Ok(user)
        }
        Err(_) => {
            let user_id = format!("u_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
            let user = User {
                id: user_id.clone(),
                username: username.clone(),
                password: password.clone(),
                name: username.clone(),
                bio: "新しい投稿を始めよう。".to_string(),
                followers: 0,
                following: 0,
            };

            conn.execute(
                "INSERT INTO users (id, username, password, name, bio, followers, following, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &user.id,
                    &user.username,
                    &user.password,
                    &user.name,
                    &user.bio,
                    user.followers as i64,
                    user.following as i64,
                    now_stamp(),
                ],
            )
            .map_err(|e| e.to_string())?;

            Ok(user)
        }
    }
}

#[tauri::command]
fn get_posts() -> Result<Vec<Post>, String> {
    let conn = open_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, author_id, author_name, handle, content, created_at, likes, retweets, replies FROM posts ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], post_from_row)
        .map_err(|e| e.to_string())?;

    let mut posts = Vec::new();
    for row in rows {
        posts.push(row.map_err(|e| e.to_string())?);
    }

    Ok(posts)
}

#[tauri::command]
fn create_post(author_id: String, content: String) -> Result<Post, String> {
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err("投稿内容を入力してください".to_string());
    }

    let conn = open_db().map_err(|e| e.to_string())?;

    let author = conn
        .query_row(
            "SELECT id, username, password, name, bio, followers, following FROM users WHERE id = ?1",
            [&author_id],
            user_from_row,
        )
        .map_err(|_| "ユーザーが見つかりませんでした".to_string())?;

    let created_at = now_stamp();
    let post_id = format!("p_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
    let handle = format!("@{}", author.username);

    conn.execute(
        "INSERT INTO posts (id, author_id, author_name, handle, content, created_at, likes, retweets, replies) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &post_id,
            &author_id,
            &author.name,
            &handle,
            &trimmed,
            &created_at,
            0,
            0,
            0,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(Post {
        id: post_id,
        author_id,
        author_name: author.name,
        handle,
        content: trimmed,
        created_at,
        likes: 0,
        retweets: 0,
        replies: 0,
    })
}

#[tauri::command]
fn toggle_like(post_id: String, user_id: String) -> Result<Post, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM likes WHERE user_id = ?1 AND post_id = ?2)",
            [&user_id, &post_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "DELETE FROM likes WHERE user_id = ?1 AND post_id = ?2",
            params![&user_id, &post_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE posts SET likes = likes - 1 WHERE id = ?1",
            [&post_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let like_id = format!("l_{}", now_stamp());
        conn.execute(
            "INSERT INTO likes (id, user_id, post_id) VALUES (?1, ?2, ?3)",
            params![&like_id, &user_id, &post_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE posts SET likes = likes + 1 WHERE id = ?1",
            [&post_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let updated = conn
        .query_row(
            "SELECT id, author_id, author_name, handle, content, created_at, likes, retweets, replies FROM posts WHERE id = ?1",
            [&post_id],
            post_from_row,
        )
        .map_err(|_| "投稿が見つかりませんでした".to_string())?;

    Ok(updated)
}

#[tauri::command]
fn toggle_retweet(post_id: String, user_id: String) -> Result<Post, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM retweets WHERE user_id = ?1 AND post_id = ?2)",
            [&user_id, &post_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "DELETE FROM retweets WHERE user_id = ?1 AND post_id = ?2",
            params![&user_id, &post_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE posts SET retweets = retweets - 1 WHERE id = ?1",
            [&post_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let retweet_id = format!("r_{}", now_stamp());
        conn.execute(
            "INSERT INTO retweets (id, user_id, post_id) VALUES (?1, ?2, ?3)",
            params![&retweet_id, &user_id, &post_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE posts SET retweets = retweets + 1 WHERE id = ?1",
            [&post_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let updated = conn
        .query_row(
            "SELECT id, author_id, author_name, handle, content, created_at, likes, retweets, replies FROM posts WHERE id = ?1",
            [&post_id],
            post_from_row,
        )
        .map_err(|_| "投稿が見つかりませんでした".to_string())?;

    Ok(updated)
}

#[tauri::command]
fn get_profile(user_id: String) -> Result<ProfileData, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let user = conn
        .query_row(
            "SELECT id, username, password, name, bio, followers, following FROM users WHERE id = ?1",
            [&user_id],
            user_from_row,
        )
        .map_err(|_| "ユーザーが見つかりません".to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, author_id, author_name, handle, content, created_at, likes, retweets, replies FROM posts WHERE author_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([&user_id], post_from_row)
        .map_err(|e| e.to_string())?;

    let mut posts = Vec::new();
    for row in rows {
        posts.push(row.map_err(|e| e.to_string())?);
    }

    Ok(ProfileData { user, posts })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            login,
            get_posts,
            create_post,
            toggle_like,
            toggle_retweet,
            get_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
