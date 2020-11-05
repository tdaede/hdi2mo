extern crate clap;
use clap::{Arg, Command};
extern crate byteorder;
use std::fs::File;
use std::path::PathBuf;
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;
use byteorder::{ByteOrder, LE};

fn main() -> io::Result<()>{
    let m = Command::new("hdi2mo")
        .about("Convert hdi image file to a MO-compatible FAT16 file system")
        .arg(Arg::new("in_file").index(1).value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("mo_file").index(2).value_parser(clap::value_parser!(PathBuf)))
        .after_help("Longer explanation to appear after the options when \
                     displaying the help information from --help or -h")
    .get_matches();
    let mut hdi_file = File::open(m.get_one::<PathBuf>("in_file").unwrap())?;
    let mut mo_file = File::create(m.get_one::<PathBuf>("mo_file").unwrap())?;
    let mut template_file = File::open("/home/thomas/sandbox/hdi2mo/formatted_mo.img")?;
    hdi_file.seek(SeekFrom::Current(0x1000))?; // skip 4096 byte header
    let mut fat16_header = [0; 512];
    template_file.read(&mut fat16_header)?;
    let mut hdi_ipl = [0; 512];
    hdi_file.read(&mut hdi_ipl)?;
    if (hdi_ipl[0xfe] != 0x55) || (hdi_ipl[0xff] != 0xaa) {
        panic!("Couldn't find NEC partition magic");
    }
    let mut hdi_part_table = [0; 512];
    hdi_file.read(&mut hdi_part_table)?;
    hdi_file.seek(SeekFrom::Current(0x8400))?; // skip mbr data and other trash
    let mut hdi_fat16_header = [0; 512];
    hdi_file.read(&mut hdi_fat16_header)?;

    let bytes_per_sector = LE::read_u16(&hdi_fat16_header[0xb..0xd]);
    let sectors_per_cluster = hdi_fat16_header[0xd];
    let reserved_sectors = LE::read_u16(&hdi_fat16_header[0xe..0x10]);
    let total_logical_sectors = LE::read_u16(&hdi_fat16_header[0x13..0x15]);
    let sectors_per_fat = LE::read_u16(&hdi_fat16_header[0x16..0x18]);
    println!("bytes per sector: {}", bytes_per_sector);
    println!("sectors per cluster: {}", sectors_per_cluster);
    println!("reserved sectors: {}", reserved_sectors);
    println!("total logical sectors: {}", total_logical_sectors);
    println!("sectors per fat: {}", sectors_per_fat);
    let bytes_per_cluster = bytes_per_sector * sectors_per_cluster as u16;
    println!("bytes per cluster: {}", bytes_per_cluster);

    LE::write_u16(&mut fat16_header[0xb..0xd], bytes_per_sector);
    fat16_header[0x0D] = sectors_per_cluster; // sectors per cluster
    //fat16_header[0x10] = 0x01; // one FAT
    //fat16_header[0x16] = 0x08; // sectors in FAT
    //fat16_header[0x17] = 0x00;
    LE::write_u16(&mut fat16_header[0x16..0x18], sectors_per_fat);
    //fat16_header[0x20] = 0xd1;
    //fat16_header[0x21] = 0x22;
    //fat16_header[0x22] = 0x00;
    //fat16_header[0x23] = 0x00;
    LE::write_u32(&mut fat16_header[0x20..0x24], total_logical_sectors as u32);
    mo_file.write(&fat16_header)?;
    // copy fat tables verbatim
    let mut fat_data = [0; 0x2000];
    hdi_file.read(&mut fat_data)?;
    mo_file.write(&fat_data)?;
    // finally write directory and file data
    let mut clusters = vec![];
    hdi_file.read_to_end(&mut clusters)?;
    mo_file.write(&clusters)?;
    mo_file.sync_all()?;
    Ok(())
}
