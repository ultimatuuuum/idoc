use clap::{ArgGroup, Parser};
use encoding_rs::EUC_KR;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[command(
    version,
    about,
    long_about = "A CLI tool to compile and decompile .ido files for game UI resources. Supports EUC-KR encoding and zlib compression. Useful for modifying game UI assets."
)]
#[command(group(
    ArgGroup::new("action")
        .required(true)
        .args(["decompile", "compile"]),
))]
struct Args {
    #[arg(short, long, help = "Decompile .ido file")]
    decompile: bool,

    #[arg(short, long, help = "Compile .xml file to .ido")]
    compile: bool,

    #[arg(short, long, help = "Input .ido file")]
    file: PathBuf,

    #[arg(short, long, help = "Output file path")]
    output: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let result = if args.compile {
        compile(&args.file, &args.output)
    } else {
        decompile(&args.file, &args.output)
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

fn decompile(path: &PathBuf, output: &PathBuf) -> Result<(), io::Error> {
    let mut file = File::open(path)?;

    let mut header = [0u8; 0x5F];
    file.read_exact(&mut header)?;

    let header_hex = hex::encode(header);

    // Decompress
    let mut decoder = ZlibDecoder::new(file);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;

    // Decode EUC-KR to UTF-8
    let (cow, _encoding_used, had_errors) = EUC_KR.decode(&decompressed_data);

    if had_errors {
        println!("Warning: Some characters could not be decoded perfectly.");
    }

    let final_xml = format!("{}\n<!-- IDO HEADER: {} -->", cow, header_hex);

    // Save the XML
    let mut output_file = File::create(output)?;
    output_file.write_all(final_xml.as_bytes())?;

    Ok(())
}

fn compile(input: &PathBuf, output: &PathBuf) -> Result<(), io::Error> {
    let header_marker = "<!-- IDO HEADER: ";
    let end_header_marker = " -->";

    println!("Reading and encoding XML from {}...", input.display());
    let mut xml_content = String::new();
    let mut input_file = File::open(input)?;
    input_file.read_to_string(&mut xml_content)?;

    let start_idx = xml_content.rfind(header_marker).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Header marker not found in XML")
    })?;
    let after_marker = &xml_content[start_idx + header_marker.len()..];
    let end_idx = after_marker.find(end_header_marker).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "End header marker not found in XML",
        )
    })?;
    let hex_str = &after_marker[..end_idx];
    let header = hex::decode(hex_str).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to decode header: {}", e),
        )
    })?;

    println!("Header recovered: {} bytes", header.len());

    // 1. Encode UTF-8 back to EUC-KR
    // If we don't do this, Korean characters will break in-game.
    let clean_xml_content = &xml_content[..start_idx].trim();
    let (cow, _, unmappable) = EUC_KR.encode(clean_xml_content);

    if unmappable {
        eprintln!("Warning: Some characters could not be mapped to EUC-KR.");
    }

    let raw_bytes = cow.to_vec();

    print!("Compressing {} bytes of EUC-KR data...", raw_bytes.len());

    let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw_bytes)?;
    let compressed_data = encoder.finish()?;

    println!("Done ({} bytes)", compressed_data.len());

    println!("Writing output file {}...", output.display());
    let mut output_file = File::create(output)?;

    output_file.write_all(&header)?;
    output_file.write_all(&compressed_data)?;

    println!(
        "Successfully compiled IDO file with compressed data ({} bytes) to {}.",
        compressed_data.len(),
        output.display()
    );

    Ok(())
}
