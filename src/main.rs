extern crate clap;
use clap::{Arg, Command};
use std::fs::File;
use std::path::PathBuf;
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;

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
    let mut template_file = File::open("/home/thomas/formatted_mo.img")?;
    hdi_file.seek(SeekFrom::Current(0x1000))?; // skip 4096 byte header
    let mut fat16_header = [0; 512];
    template_file.read(&mut fat16_header)?;
    //fat16_header[0x0B] = 0x00; // logical sector size
    //fat16_header[0x0C] = 0x04;
    //fat16_header[0x0D] = 0x02; // sectors per cluster
    //fat16_header[0x10] = 0x01; // one FAT
    fat16_header[0x16] = 0x08; // sectors in FAT
    fat16_header[0x17] = 0x00;
    fat16_header[0x20] = 0xd1;
    fat16_header[0x21] = 0x22;
    fat16_header[0x22] = 0x00;
    fat16_header[0x23] = 0x00;
    mo_file.write(&fat16_header)?;
    //mo_file.write(&[0;512])?; // extra padding for 1st 1024 byte sector
    hdi_file.seek(SeekFrom::Current(0x8800))?; // skip mbr data and other trash
    hdi_file.seek(SeekFrom::Current(0x400))?; // skip fat16 header
    // copy fat tables verbatim
    let mut fat_data = [0; 0x2000];
  hdi_file.read(&mut fat_data)?;
  fat_data[0] = 0xF0;
    mo_file.write(&fat_data)?;
    // now we need to write more garbage fat data
    //mo_file.write(&[0; 0x32600])?;
    //mo_file.write(&[0; 0x6f200])?;
    // finally write directory and file data
    let mut clusters = vec![];
    hdi_file.read_to_end(&mut clusters)?;
    mo_file.write(&clusters)?;
    mo_file.sync_all()?;
    Ok(())
}
