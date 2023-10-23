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
struct HDIHeader {
    reserved: u32,
    pda: u32,
    header_size: u32,
    data_size: u32,
    bytes_per_sector: u32,
    sectors: u32,
    heads: u32,
    cylinders: u32,
}

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

#[binrw]
#[brw(little)]
#[derive(Debug, Copy, Clone)]
struct BIOSParameterBlock {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    max_root_dirents: u16,
    total_logical_sectors: u16,
    media_id: u8,
    sectors_per_fat: u16,
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
    let hdi_header = HDIHeader::read(&mut hdi_file).unwrap();
    hdi_file.seek(SeekFrom::Start(hdi_header.header_size as u64))?;
    let mut fat16_header = [0; 512];
    template_file.read_exact(&mut fat16_header)?;
    let mut hdi_ipl = [0; 512];
    hdi_file.read_exact(&mut hdi_ipl)?;
    if (hdi_ipl[0xfe] != 0x55) || (hdi_ipl[0xff] != 0xaa) {
        panic!("Couldn't find NEC partition magic");
    }
    println!("hdd sector size: {}", hdi_header.bytes_per_sector);
    let hdi_part_table = PC98PartitionTable::read(&mut hdi_file).unwrap();
    for p in hdi_part_table.partitions {
        if p.mid != 0 {
            println!("found partition: {:?}", std::str::from_utf8(&p.name).unwrap());
        }
    }
    let p = hdi_part_table.partitions[0];
    let p_start_offset = hdi_header.header_size as u64
                                  + ((p.scyl as u64 * hdi_header.heads as u64 + p.shd as u64)
                                     * hdi_header.sectors as u64 + p.ssect as u64)
                                  * hdi_header.bytes_per_sector as u64;
    hdi_file.seek(SeekFrom::Start(p_start_offset))?;
    let mut hdi_fat16_header = [0; 512];
    hdi_file.read_exact(&mut hdi_fat16_header)?;
    hdi_file.seek(SeekFrom::Start(p_start_offset + 0x0b))?;

    let bpb = BIOSParameterBlock::read(&mut hdi_file).unwrap();

    println!("bytes per sector: {}", bpb.bytes_per_sector);
    println!("sectors per cluster: {}", bpb.sectors_per_cluster);
    println!("reserved sectors: {}", bpb.reserved_sectors);
    assert_eq!(bpb.reserved_sectors, 1);
    println!("total logical sectors: {}", bpb.total_logical_sectors);
    println!("sectors per fat: {}", bpb.sectors_per_fat);
    let bytes_per_cluster = bpb.bytes_per_sector as usize* bpb.sectors_per_cluster as usize;
    println!("bytes per cluster: {}", bytes_per_cluster);
    println!("max root dirents: {}", bpb.max_root_dirents);

    let mo_bytes_per_sector = bpb.bytes_per_sector;
    let mo_total_logical_sectors = bpb.total_logical_sectors as u64 * bpb.bytes_per_sector as u64 / mo_bytes_per_sector as u64;
    let mo_sectors_per_fat = (bpb.sectors_per_fat * bpb.bytes_per_sector / mo_bytes_per_sector) as u16;

    LE::write_u16(&mut fat16_header[0xb..0xd], mo_bytes_per_sector as u16);
    fat16_header[0x0D] = (bytes_per_cluster / mo_bytes_per_sector as usize) as u8; // sectors per cluster
    //fat16_header[0x10] = 0x01; // one FAT
    LE::write_u16(&mut fat16_header[0x11..0x13], bpb.max_root_dirents);
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
    hdi_file.seek(SeekFrom::Start(bpb.reserved_sectors as u64 * bpb.bytes_per_sector as u64 + p_start_offset))?;
    // write reserved sector if necessary
    let reserved_bytes = vec![0; bpb.reserved_sectors as usize * mo_bytes_per_sector as usize - 512];
    mo_file.write(&reserved_bytes)?;
    // copy fat tables verbatim
    let mut fat_data = vec![0; bpb.num_fats as usize * bpb.sectors_per_fat as usize * bpb.bytes_per_sector as usize];
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
