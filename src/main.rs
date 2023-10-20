extern crate clap;
use clap::{Arg, Command};
extern crate byteorder;
extern crate binrw;

use std::fs::File;
use std::path::PathBuf;
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;
use byteorder::{ByteOrder, LE};
use binrw::{binrw, BinRead};

#[binrw]
#[brw(little)]
#[derive(Debug, Copy, Clone)]
struct PC98Partition {
    mid: u8,
    sid: u8,
    dummy1: u8,
    dummy2: u8,
    ipl_sct: u8,
    ipl_head: u8,
    ipl_cyl: u16,
    ssect: u8,
    shd: u8,
    scyl: u16,
    esect: u8,
    ehd: u8,
    ecyl: u16,
    name: [u8; 16],
}

#[binrw]
#[brw(little)]
#[derive(Debug, Copy, Clone)]
struct PC98PartitionTable {
    partitions: [PC98Partition; 16],
}

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
    template_file.read_exact(&mut fat16_header)?;
    let mut hdi_ipl = [0; 512];
    hdi_file.read_exact(&mut hdi_ipl)?;
    if (hdi_ipl[0xfe] != 0x55) || (hdi_ipl[0xff] != 0xaa) {
        panic!("Couldn't find NEC partition magic");
    }
    let hdi_part_table = PC98PartitionTable::read(&mut hdi_file).unwrap();
    for p in hdi_part_table.partitions {
        if p.mid != 0 {
            println!("found partition: {:?}", std::str::from_utf8(&p.name).unwrap());
        }
    }
    hdi_file.seek(SeekFrom::Current(0x8400))?; // skip mbr data and other trash
    let mut hdi_fat16_header = [0; 512];
    hdi_file.read_exact(&mut hdi_fat16_header)?;

    let bytes_per_sector = LE::read_u16(&hdi_fat16_header[0xb..0xd]) as u32;
    let sectors_per_cluster = hdi_fat16_header[0xd] as u32;
    let reserved_sectors = LE::read_u16(&hdi_fat16_header[0xe..0x10]);
    let total_logical_sectors = LE::read_u16(&hdi_fat16_header[0x13..0x15]) as u32;
    let sectors_per_fat = LE::read_u16(&hdi_fat16_header[0x16..0x18]) as u32;
    let max_root_dirents = LE::read_u16(&hdi_fat16_header[0x11..0x13]);
    let num_fats = 2;
    println!("bytes per sector: {}", bytes_per_sector);
    println!("sectors per cluster: {}", sectors_per_cluster);
    println!("reserved sectors: {}", reserved_sectors);
    assert_eq!(reserved_sectors, 1);
    println!("total logical sectors: {}", total_logical_sectors);
    println!("sectors per fat: {}", sectors_per_fat);
    let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
    println!("bytes per cluster: {}", bytes_per_cluster);
    println!("max root dirents: {}", max_root_dirents);

    let mo_bytes_per_sector = bytes_per_sector;
    let mo_total_logical_sectors = total_logical_sectors * bytes_per_sector / mo_bytes_per_sector;
    let mo_sectors_per_fat = (sectors_per_fat * bytes_per_sector / mo_bytes_per_sector) as u16;

    LE::write_u16(&mut fat16_header[0xb..0xd], mo_bytes_per_sector as u16);
    fat16_header[0x0D] = (bytes_per_cluster / mo_bytes_per_sector) as u8; // sectors per cluster
    //fat16_header[0x10] = 0x01; // one FAT
    LE::write_u16(&mut fat16_header[0x11..0x13], max_root_dirents);
    //fat16_header[0x16] = 0x08; // sectors in FAT
    //fat16_header[0x17] = 0x00;
    LE::write_u16(&mut fat16_header[0x16..0x18], mo_sectors_per_fat);
    //fat16_header[0x20] = 0xd1;
    //fat16_header[0x21] = 0x22;
    //fat16_header[0x22] = 0x00;
    //fat16_header[0x23] = 0x00;
    LE::write_u32(&mut fat16_header[0x20..0x24], mo_total_logical_sectors as u32);
    mo_file.write(&fat16_header)?;
    // skip past first reserved sector
    hdi_file.seek(SeekFrom::Current(reserved_sectors as i64 * bytes_per_sector as i64 - 512))?;
    // write reserved sector if necessary
    let reserved_bytes = vec![0; reserved_sectors as usize * mo_bytes_per_sector as usize - 512];
    mo_file.write(&reserved_bytes)?;
    // copy fat tables verbatim
    let mut fat_data = vec![0; num_fats * sectors_per_fat as usize * bytes_per_sector as usize];
    hdi_file.read_exact(&mut fat_data)?;
    //fat_data[0] = 0xF0;
    mo_file.write(&fat_data)?;
    // finally write directory and file data
    let mut clusters = vec![];
    hdi_file.read_to_end(&mut clusters)?;
    mo_file.write(&clusters)?;
    mo_file.sync_all()?;
    Ok(())
}
