use byteorder::{LittleEndian, ReadBytesExt};
use clap::{ArgGroup, Parser};
use encoding_rs::EUC_KR;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde::Serialize;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[command(
    version,
    about,
    long_about = "A CLI tool to compile and decompile .ido files. Supports EUC-KR encoding and zlib compression."
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

    if header.starts_with(&[0x01, 0x00, 0x01, 0x00]) {
        println!("Detected Type: Shop Database (Binary Structs)");
        // We change the output extension to .csv automatically if user didn't specify

        return parse_shop_db(&path, &output);
    }

    let header_hex = hex::encode(header);

    // Decompress
    let mut decoder = ZlibDecoder::new(file);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;

    let extension = if decompressed_data.starts_with(b"DDS ") {
        println!("Detected Type: DDS Texture");
        Some("dds")
    } else if decompressed_data.ends_with(b"TRUEVISION-XFILE.\0") {
        println!("Detected Type: TGA Texture");
        Some("tga")
    } else if decompressed_data.starts_with(b"BM") {
        println!("Detected Type: BMP Texture");
        Some("bmp")
    } else if decompressed_data.starts_with(b"\x89PNG") {
        println!("Detected Type: PNG Texture");
        Some("png")
    } else {
        None
    };

    if let Some(extension) = extension {
        let output_path = if output.extension().is_none() {
            let output_w_ext = output.with_extension(extension);
            output_w_ext
        } else {
            output.clone()
        };

        let mut output_file = File::create(&output_path)?;
        output_file.write_all(&decompressed_data)?;

        println!("Saved as {}", output_path.display());
        return Ok(());
    }

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

fn parse_shop_db(input: &PathBuf, output: &PathBuf) -> io::Result<()> {
    println!(
        "Parsing Shop Database: {} -> {}",
        input.display(),
        output.display()
    );

    let mut file = File::open(input)?;
    let file_len = file.metadata()?.len();
    let record_size = 456; // 0x1C8 from Node.js script

    if file_len % record_size != 0 {
        println!(
            "Warning: File size is not a multiple of record size ({})!",
            record_size
        );
    }

    let item_count = file_len / record_size;
    println!("Found {} items.", item_count);

    let mut items = Vec::new();

    for i in 0..item_count {
        let offset = i * record_size;
        file.seek(SeekFrom::Start(offset))?;

        // Read Fields (Offsets from Node.js script)
        // 0x00: Category
        let category = file.read_u16::<LittleEndian>()?;
        // 0x02: Type ID
        let item_type_id = file.read_u16::<LittleEndian>()?;
        // 0x04: Variant ID
        let variant_id = file.read_i16::<LittleEndian>()?;
        // 0x06: Validity
        let validity = file.read_i16::<LittleEndian>()?;

        // Skip to 0x0C: Type Flag
        file.seek(SeekFrom::Start(offset + 0x0C))?;
        let type_flag = file.read_u8()?;

        // Skip to 0x38: Set Item ID
        file.seek(SeekFrom::Start(offset + 0x38))?;
        let set_item_id = file.read_i32::<LittleEndian>()?;

        // Skip to 0x64: Name (100 bytes / 50 wchars)
        file.seek(SeekFrom::Start(offset + 0x64))?;
        let mut name_buffer = [0u8; 100];
        file.read_exact(&mut name_buffer)?;

        // Parse UTF-16LE String
        let name = parse_utf16_string(&name_buffer);

        items.push(ShopItem {
            category,
            item_type_id,
            variant_id,
            validity,
            type_flag,
            set_item_id,
            name,
        });
    }

    // Write to CSV
    let mut wtr = csv::Writer::from_path(output)?;
    for item in items {
        wtr.serialize(item)?;
    }
    wtr.flush()?;

    println!("Success! Dumped to {}", output.display());
    Ok(())
}

fn parse_utf16_string(buffer: &[u8]) -> String {
    let u16_vec: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|&c| c != 0) // Stop at null terminator
        .collect();

    String::from_utf16_lossy(&u16_vec).trim().to_string()
}

#[derive(Debug, Serialize)]
struct ShopItem {
    category: u16,
    item_type_id: u16,
    variant_id: i16,
    validity: i16,
    type_flag: u8,
    set_item_id: i32,
    name: String,
}
