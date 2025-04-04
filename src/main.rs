use anyhow::{anyhow, Result};
use libflate::gzip::Decoder;
use mailparse::*;
use native_tls::TlsConnector;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use structopt::StructOpt;
use zip::ZipArchive;
extern crate libflate;
extern crate rpassword;

#[derive(Debug, StructOpt)]
/// imap_dmarc_extractor
///
/// Will connect to an IMAP server and try to extract all DMARC reports,
/// usually xml files stored in a gzip or zip file
struct Opt {
    /// IMAP Server
    /// mail.example.com:993
    #[structopt()]
    server: String,

    /// Username for the IMAP account
    #[structopt()]
    account: String,

    /// Password for the IMAP account
    #[structopt(short, long)]
    _password: Option<String>,

    /// Path where to store the reports
    #[structopt(parse(from_os_str))]
    path: PathBuf,
}

struct Attachment {
    content: Vec<u8>,
    decompressed: Option<Vec<u8>>,
    mimetype: String,
    name: String,
}

const USABLE_MIMETYPES: [&str; 3] = [
    "application/zip",
    "application/gzip",
    "application/octet-stream",
];

fn main() {
    let opt = Opt::from_args();

    let v: Vec<&str> = opt.server.split(':').collect();
    let account = opt.account;
    let path = opt.path;
    let server = v[0];
    let mut port = 993;

    if v.len() > 1 {
        port = v[1].parse().unwrap();
    }

    let password = rpassword::prompt_password_stdout("Password: ").unwrap();

    println!(
        "Will connect to {} on port {} with account '{}'",
        server, port, account
    );
    let tls = TlsConnector::builder().build().unwrap();
    let client = imap::connect((server, port), server, &tls).expect("Error connecting to server");
    let mut imap_session = client.login(account, password).unwrap();

    let inbox = imap_session.select("INBOX").unwrap();
    let message_count = inbox.exists;
    let messages = imap_session.fetch("1:*", "RFC822").unwrap();

    println!("Connected to IMAP server.");

    for message in messages.iter() {
        println!(
            "{:.2} % done",
            100.00 / message_count as f32 * message.message as f32
        );
        if let Some(body) = message.body() {
            let mail = parse_mail(body).unwrap();
            let message_id = mail.headers.get_first_value("Message-ID").unwrap();

            let attachment = match get_attachment(&mail) {
                Ok(attachment) => attachment,
                Err(e) => {
                    eprintln!("{} Message: {}", e, message_id);
                    continue;
                }
            };

            let attachment = decompress_attachment(attachment).unwrap();

            let mut filepath = path.clone();
            filepath.push(attachment.name.clone());
            let mut file = File::create(&filepath).expect("Could not create file.");
            match file.write_all(&attachment.decompressed.unwrap()) {
                Ok(()) => (),
                Err(e) => eprintln!("{}", e),
            };
        }
    }
    imap_session.logout().unwrap();
    println!("Finished!");
}

fn decompress_attachment(mut attachment: Attachment) -> Result<Attachment> {
    // Decompresses the attachment, saves it in te Attachment struct and returns it

    let content = std::io::Cursor::new(&attachment.content);
    let mut decompressed: Vec<u8> = Vec::new();
    // TODO: add function that determines type better, e.g. check file extension if mimetype is
    // octect stream
    if attachment.mimetype == *"application/zip" {
        let mut zip = ZipArchive::new(content).unwrap();
        let mut report = zip.by_index(0)?;
        std::io::copy(&mut report, &mut decompressed)?;
        attachment.name = String::from(report.name());
    } else if attachment.mimetype == *"application/gzip"
        || attachment.mimetype == *"application/octet-stream"
    {
        let mut report = Decoder::new(content).unwrap();
        std::io::copy(&mut report, &mut decompressed)?;
        let mut path = PathBuf::from(attachment.name.clone());
        path = path.with_extension("");
        attachment.name = String::from(path.to_str().unwrap());
    }
    attachment.decompressed = Some(decompressed);

    Ok(attachment)
}

fn get_attachment(mail: &ParsedMail) -> Result<Attachment> {
    // Extracts the attachment from the mail

    let mut content_type = mail.ctype.mimetype.clone();
    let mut body: Vec<u8> = vec![];
    let mut name = String::new();

    if USABLE_MIMETYPES.contains(&content_type.as_str()) {
        body = mail.get_body_raw().unwrap().clone();
        name = mail
            .get_content_disposition()
            .params
            .get("filename")
            .unwrap()
            .clone();
    } else if !mail.subparts.is_empty() {
        for subpart in &mail.subparts {
            content_type = subpart.ctype.mimetype.clone();
            if USABLE_MIMETYPES.contains(&content_type.as_str()) {
                body = subpart.get_body_raw()?;
                name = subpart
                    .get_content_disposition()
                    .params
                    .get("filename")
                    .unwrap()
                    .clone();
                break;
            }
        }
    }

    if body.is_empty() {
        return Err(anyhow!("No attachment found."));
    }
    if name.is_empty() {
        return Err(anyhow!("No file name found."));
    }

    Ok(Attachment {
        content: body,
        decompressed: None,
        name,
        mimetype: content_type,
    })
}
