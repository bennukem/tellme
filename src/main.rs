use actix_web::{post, delete, web, App, HttpResponse, HttpServer, Responder};
use validator::Validate;
use serde::{Serialize, Deserialize};
use sqlx::{migrate::MigrateDatabase, FromRow, Sqlite, SqlitePool, Pool};
use tokio::sync::mpsc::{self, Sender};
use actix_cors::Cors;
use nanoid::nanoid;
use lettre::{Message, message::header::ContentType, transport::smtp::authentication::Credentials, SmtpTransport, Transport};
use dotenv::dotenv;

#[derive(Deserialize, Validate)]
struct FormAccount {
    #[validate(email)]
    email: String,
}

#[derive(FromRow, Serialize)]
struct Account {
    token: String,
    email: String,
    counter: u32,
}

#[derive(Deserialize, Validate)]
struct FormMessageData {
    #[validate(length(max = 21))]
    token: String,
    #[validate(length(max = 64))]
    first_name: Option<String>,
    #[validate(length(max = 64))]
    last_name: Option<String>,
    #[validate(length(max = 128))]
    subject: Option<String>,
    #[validate(email)]
    email: String,
    #[validate(length(min = 10, max = 2048))]
    body: String,
}

// POST Generer un compte qui retournera un token
#[post("/account")]
async fn create_account(data: web::Json<FormAccount>, pool: web::Data<Pool<Sqlite>>) -> impl Responder {
    if let Err(err) = data.validate() {
        return HttpResponse::BadRequest().json(err);
    }

    // On va tout de même vérifier qu'un compte n'existe pas déjà pour l'email donné.
    match sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE email=?")
        .bind(&data.email)
        .fetch_optional(pool.get_ref())
        .await
        .unwrap() {
            // Si le compte existe déjà, on le retourne.
            Some(account) => HttpResponse::Ok().json(account),
            // Sinon, on le créé.
            None => {
                let account = Account {
                    token: nanoid!(),
                    email: data.email.clone(),
                    counter: 0,
                };
                
                sqlx::query("INSERT INTO accounts (token, email) VALUES ($1, $2)")
                    .bind(&account.token)
                    .bind(&account.email)
                    .execute(pool.get_ref())
                    .await.unwrap();

                    HttpResponse::Ok().json(account)
            },
    }
}

// POST Envoyer form avec en paramètre le token
#[post("/message")]
async fn create_message(data: web::Json<FormMessageData>, pool: web::Data<Pool<Sqlite>>, tx: web::Data<Sender<Message>>) -> impl Responder {
    if let Err(err) = data.validate() {
        return HttpResponse::BadRequest().json(err);
    }

    let account = sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE token=?")
    .bind(&data.token)
    .fetch_optional(pool.as_ref())
    .await
    .unwrap();

    match account {
        Some(account) => {
            sqlx::query("UPDATE accounts set counter=counter+1, last_action=? WHERE token=?")
                .bind(chrono::Utc::now())
                .bind(&account.token)
                .execute(pool.as_ref())
                .await.unwrap();

            let message = Message::builder()
                .from(format!("<{}>", std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set.")).parse().unwrap())
                .reply_to(format!("{}<{}>", concat_name(&data.first_name, &data.last_name).unwrap_or("".to_owned()), data.email).parse().unwrap())
                .to(format!("<{}>", account.email).parse().unwrap())
                .header(ContentType::TEXT_PLAIN)
                .subject(format!("{}", data.subject.clone().unwrap_or("No subject".to_string())))
                .body(data.body.to_owned()).unwrap();

            // Send the Message to the queue
            let _ = tx.send(message).await;
            println!("Message for : {:?}", account.email);
            HttpResponse::Ok().body("OK")
        },
        None => {
            HttpResponse::NotFound().body("Invalid token")
        },
    }
}

// DELETE Supprimer un compte via le combo email token
#[delete("/account")]
async fn delete_account(data: web::Json<FormAccount>, pool: web::Data<Pool<Sqlite>>) -> impl Responder {
    if let Err(err) = data.validate() {
        return HttpResponse::BadRequest().json(err);
    }

    match sqlx::query("DELETE FROM accounts WHERE email=?")
        .bind(&data.email)
        .execute(pool.as_ref())
        .await.unwrap().rows_affected() {

        1 => HttpResponse::Ok().body("DELETED"),
        _ => HttpResponse::NotFound().body("Email not found"),
    }
}

// Le process qui enverra les emails
async fn send_email_workers(mut rx: mpsc::Receiver<Message>) {
    // Create mailer
    let creds = Credentials::new(std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set."), std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set."));

    // Open a remote connection to smtp server
    let mailer = SmtpTransport::relay(std::env::var("SMTP_HOSTNAME").expect("SMTP_HOSTNAME must be set.").as_str())
        .unwrap()
        .credentials(creds)
        .build();

    while let Some(message) = rx.recv().await {
        // Send email
        mailer.send(&message).unwrap();
    }
}

// Générer un nom falide, a repasser à la mouilinette via un to string ?
fn concat_name(first_name: &Option<String>, last_name: &Option<String>) -> Option<String> {
    if first_name.is_some() && last_name.is_some() {
        return Some(format!("{} {}", first_name.clone().unwrap(), last_name.clone().unwrap()))
    } else if first_name.is_some() {
        return Some(format!("{}", first_name.clone().unwrap()))
    } else if last_name.is_some() {
        return Some(format!("{}", last_name.clone().unwrap()))
    }

    None
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    
    // Chemin de la base de données.
    // Pervoir de passer via CLAP ou .env
    let db_url = "sqlite://var/db.sqlite3";

    // Créer la base de données si elle n'existe pas.
    match Sqlite::create_database(db_url).await {
        Ok(_) => println!("Database connection successful"),
        Err(error) => panic!("error: {}", error),
    }

    // Connexion à la base de données.
    let db_pool = SqlitePool::connect(db_url).await.unwrap();

    // Créer la table si besoin
    // Je souhaite avec une relation entre un token ET un email. 
    // Le token servira de primary key
    match sqlx::query("CREATE TABLE IF NOT EXISTS accounts (token BLOB PRIMARY KEY NOT NULL, email VARCHAR(250) NOT NULL, counter INTEGER DEFAULT 0 NOT NULL, last_action TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL);").execute(&db_pool).await {
        Err(error) => panic!("error: {}", error),
        _ => (),
    }

    // Initialisation du channel de communication entre les threads.
    let (tx, rx) = mpsc::channel::<Message>(1024);
    tokio::spawn(async move {
        send_email_workers(rx).await;
    });
        
    HttpServer::new(move || {
        // On autorise tout venant de partout :)
        let cors = Cors::default().allow_any_origin().allow_any_header().allow_any_method().send_wildcard();

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(web::Data::new(tx.clone()))
            .service(create_account)
            .service(delete_account)
            .service(create_message)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}