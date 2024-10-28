use clap::{ArgEnum, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use select::document::Document;
use select::node::Node;
use select::predicate::Name;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use zip::{write::FileOptions, ZipWriter};

#[derive(Parser)]
#[clap(name = "manga-cli")]
#[clap(about = "A command-line manga downloader.")]
struct CLI {
    #[clap(short, long, arg_enum)]
    format: Option<Format>, // Add `format` as Option<Format>

    #[clap(short, long)]
    clear: bool,

    #[clap(short, long)]
    viewer: Option<String>,

    manga_name: String,
}

#[derive(ArgEnum, Clone)]
enum Format {
    Pdf,
    Cbz,
}

const SEARCH_URL: &str = "https://m.manganelo.com/search/story/";
const IMAGE_DIR: &str = ".cache/manga-cli";

fn main() {
    let cli = CLI::parse();

    if cli.clear {
        clear_cache();
        return;
    }

    let manga_name = format_manga_name(&cli.manga_name);
    let manga_ids = fetch_manga_ids(&manga_name).expect("Failed to fetch manga IDs");

    // Display available manga titles
    for (index, title) in manga_ids.iter().enumerate() {
        println!("[{}] {}", index + 1, title);
    }

    let manga_number: usize = prompt("Enter number: ") - 1;
    let manga_link = &manga_ids[manga_number];

    let chapter_number: usize = prompt("Enter chapter number: ");
    let chapter_link = format!("{}/chapter-{}", manga_link, chapter_number);

    // Use cli.format directly, passing it as Option<Format>
    download_chapter(&chapter_link, cli.format).expect("Failed to download chapter");
}

fn fetch_manga_ids(manga_name: &str) -> Result<Vec<String>, reqwest::Error> {
    let client = Client::new();
    let response = client
        .get(format!("{}{}", SEARCH_URL, manga_name))
        .header(USER_AGENT, "Mozilla/5.0")
        .send()?
        .text()?;

    let document = Document::from(response.as_str());
    let titles: Vec<String> = document
        .find(Name("h3"))
        .filter_map(|node: Node| node.find(Name("a")).next())
        .filter_map(|node: Node| node.attr("href").map(|href| href.to_string()))
        .collect();

    Ok(titles)
}

fn download_chapter(
    chapter_link: &str,
    format: Option<Format>,
) -> Result<(), Box<dyn std::error::Error>> {
    let images = fetch_image_links(chapter_link)?;

    create_image_directory()?;
    for (i, image_url) in images.iter().enumerate() {
        let image_path = format!("{}/{}.jpg", IMAGE_DIR, i + 1);
        download_image(image_url, &image_path)?;
    }

    match format {
        Some(Format::Pdf) => create_pdf()?,
        Some(Format::Cbz) => create_cbz()?,
        None => println!("No format specified, skipping conversion."),
    }

    Ok(())
}

fn fetch_image_links(chapter_link: &str) -> Result<Vec<String>, reqwest::Error> {
    let client = Client::new();
    let response = client
        .get(chapter_link)
        .header(USER_AGENT, "Mozilla/5.0")
        .send()?
        .text()?;

    let document = Document::from(response.as_str());
    let images: Vec<String> = document
        .find(Name("img"))
        .filter_map(|node: Node| node.attr("src").map(|src| src.to_string()))
        .collect();

    Ok(images)
}

fn download_image(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::blocking::get(url)?.bytes()?;
    fs::write(path, &response)?;
    Ok(())
}

fn create_image_directory() -> std::io::Result<()> {
    fs::create_dir_all(IMAGE_DIR)?;
    Ok(())
}

fn create_pdf() -> Result<(), Box<dyn std::error::Error>> {
    println!("Converting images to PDF...");

    let mut images: Vec<String> = Vec::new();
    for i in 1..=1000 {
        let img_path = format!("{}/{}.jpg", IMAGE_DIR, i);
        if PathBuf::from(&img_path).exists() {
            images.push(img_path);
        } else {
            break; // Stop when no more numbered images are found
        }
    }

    if images.is_empty() {
        return Err("No images found to convert to PDF.".into());
    }

    let status = std::process::Command::new("magick")
        .args(["convert", "-quality", "100"])
        .args(&images)
        .arg("output.pdf")
        .current_dir(IMAGE_DIR)
        .status()?;

    if !status.success() {
        return Err("Failed to create PDF".into());
    }

    println!("PDF created successfully in {}/output.pdf", IMAGE_DIR);
    Ok(())
}

fn create_cbz() -> Result<(), Box<dyn std::error::Error>> {
    let cbz_path = format!("{}/output.cbz", IMAGE_DIR);
    let file = fs::File::create(&cbz_path)?;
    let mut zip = ZipWriter::new(file);

    for i in 1..=1000 {
        let img_path = format!("{}/{}.jpg", IMAGE_DIR, i);
        let path_buf = PathBuf::from(&img_path);
        if path_buf.exists() {
            zip.start_file(format!("{}.jpg", i), FileOptions::default())?;
            let img_data = fs::read(&img_path)?;
            zip.write_all(&img_data)?;
        } else {
            break;
        }
    }

    zip.finish()?;
    println!("CBZ created successfully.");
    Ok(())
}

fn clear_cache() {
    if fs::remove_dir_all(IMAGE_DIR).is_ok() {
        println!("Cleared cache.");
    } else {
        println!("No cache to clear.");
    }
}

fn prompt(message: &str) -> usize {
    print!("{}", message);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse().unwrap_or_else(|_| {
        println!("Invalid input, please enter a number.");
        prompt(message)
    })
}

fn format_manga_name(manga_name: &str) -> String {
    manga_name.replace(" ", "_").replace("-", "_")
}
