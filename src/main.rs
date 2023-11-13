extern crate clap;
use clap::{Arg, Command};
extern crate binrw;

use std::fs::File;
use std::path::PathBuf;
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;
use binrw::{binrw, BinRead, BinWrite};

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
    sectors_per_track: u16,
    heads: u16,
    hidden_sectors: u32,
    total_logical_sectors_u32: u32,
    physical_drive_number: u8,
    reserved_1: u8,
    extended_signature: u8,
    volume_id: [u8; 4],
    volume_label: [u8; 11],
    file_system_type: [u8; 8],
    nec_extra_reserved_sectors: u16,
    nec_cylinder_head: u16,
    nec_sector_offset_to_iosys: u16,
    nec_physical_sector_size: u16,
}

fn main() -> io::Result<()>{
    let m = Command::new("hdi2mo")
        .about("Convert hdi image file to a MO-compatible FAT16 file system")
        .arg(Arg::new("mo_template").short('t').value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("in_file").index(1).value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("mo_file").index(2).value_parser(clap::value_parser!(PathBuf)))
        .after_help("Longer explanation to appear after the options when \
                     displaying the help information from --help or -h")
    .get_matches();
    let mut hdi_file = File::open(m.get_one::<PathBuf>("in_file").unwrap())?;
    let mut mo_file = File::create(m.get_one::<PathBuf>("mo_file").unwrap())?;
    let mut template_file = if let Some(mo_template_path) = m.get_one::<PathBuf>("mo_template") {
        Some(File::open(mo_template_path)?)
    } else {
        None
    };
    let hdi_header = HDIHeader::read(&mut hdi_file).unwrap();
    hdi_file.seek(SeekFrom::Start(hdi_header.header_size as u64))?;
    let mut fat16_header = [0; 512];
    let mut hdi_ipl = vec![0; hdi_header.bytes_per_sector as usize];
    hdi_file.read_exact(&mut hdi_ipl)?;
    if (hdi_ipl[0xfe] != 0x55) || (hdi_ipl[0xff] != 0xaa) {
        panic!("Couldn't find NEC partition magic");
    }
    println!("hdd sector size: {}", hdi_header.bytes_per_sector);
    let hdi_part_table = PC98PartitionTable::read(&mut hdi_file).unwrap();
    for p in hdi_part_table.partitions {
        if p.mid != 0 {
            println!("found partition: {:?}", String::from_utf8_lossy(&p.name));
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

    println!("{:?}", bpb);
    let bytes_per_cluster = bpb.bytes_per_sector as usize * bpb.sectors_per_cluster as usize;
    assert_eq!(bpb.reserved_sectors, 1);
    assert_eq!(bpb.num_fats, 2);
    println!("bytes per cluster: {}", bytes_per_cluster);

    let t_bpb = if let Some(t) = &mut template_file {
        t.read_exact(&mut fat16_header)?;
        t.seek(SeekFrom::Start(0x0b))?;
        BIOSParameterBlock::read(t).unwrap()
    } else {
        fat16_header = hdi_fat16_header;
        // todo: not all of these are required
        fat16_header[0x8] = b'5';
        fat16_header[0xa] = b'0';
        fat16_header[0x25] = 1;
        fat16_header[0x3a] = b'6';
        fat16_header[0x3e] = 0;
        fat16_header[0x42] = 0x07;
        fat16_header[0x43] = 0x02;
        fat16_header[0x1fe] = 0x55;
        fat16_header[0x1ff] = 0xaa;

        // default values for a 128MB MO
        BIOSParameterBlock {
            bytes_per_sector: 512,
            sectors_per_cluster: 4,
            reserved_sectors: 1,
            num_fats: 2,
            max_root_dirents: 512,
            total_logical_sectors: 0,
            media_id: 240,
            sectors_per_fat: 243,
            sectors_per_track: 25,
            heads: 1,
            hidden_sectors: 0,
            total_logical_sectors_u32: 248800,
            physical_drive_number: 0,
            reserved_1: bpb.reserved_1,
            extended_signature: bpb.extended_signature,
            volume_id: bpb.volume_id,
            volume_label: bpb.volume_label,
            file_system_type: bpb.file_system_type,
            nec_extra_reserved_sectors: 0,
            nec_cylinder_head: 0,
            nec_sector_offset_to_iosys: 0,
            nec_physical_sector_size: 512,
        }
    };
    println!("{:?}", t_bpb);

    let hdi_iosys_head = bpb.nec_cylinder_head & 0xff;
    let hdi_iosys_cylinder = bpb.nec_cylinder_head >> 8;
    let hdi_iosys_offset = ((hdi_iosys_cylinder as u64 * hdi_header.heads as u64 + hdi_iosys_head as u64)
                                     * hdi_header.sectors as u64 + bpb.nec_sector_offset_to_iosys as u64 + bpb.nec_extra_reserved_sectors as u64)
                                  * hdi_header.bytes_per_sector as u64;

    let mo_bytes_per_sector = t_bpb.bytes_per_sector;
    let mo_total_logical_sectors = bpb.total_logical_sectors as u64 * bpb.bytes_per_sector as u64 / mo_bytes_per_sector as u64;
    let mo_sectors_per_fat = (bpb.sectors_per_fat * bpb.bytes_per_sector / mo_bytes_per_sector) as u16;
    let mo_reserved_bytes = bpb.bytes_per_sector as u64 * bpb.reserved_sectors as u64;

    let mut mo_bpb = t_bpb.clone();

    mo_bpb.bytes_per_sector = mo_bytes_per_sector as u16;
    mo_bpb.sectors_per_cluster = (bytes_per_cluster / mo_bytes_per_sector as usize) as u8;
    mo_bpb.max_root_dirents = bpb.max_root_dirents;
    mo_bpb.sectors_per_fat = mo_sectors_per_fat;
    mo_bpb.total_logical_sectors_u32 = mo_total_logical_sectors as u32;
    mo_bpb.reserved_sectors = (mo_reserved_bytes / mo_bytes_per_sector as u64) as u16;
    mo_bpb.nec_extra_reserved_sectors = 0;
    mo_bpb.nec_sector_offset_to_iosys = ((hdi_iosys_offset - p_start_offset + hdi_header.header_size as u64) / mo_bpb.nec_physical_sector_size as u64) as u16;
    mo_file.write_all(&fat16_header)?;
    mo_file.seek(SeekFrom::Start(0x0b))?;
    mo_bpb.write(&mut mo_file).unwrap();
    println!("{:?}", mo_bpb);
    mo_file.seek(SeekFrom::Start(512))?;
    // skip past first 512 bytes
    hdi_file.seek(SeekFrom::Start(512 + p_start_offset))?;
    // read remaining bytes of reserved sectors and write them to mo
    let mut reserved_bytes = vec![0; mo_reserved_bytes as usize - 512];
    hdi_file.read_exact(&mut reserved_bytes)?;
    mo_file.write_all(&reserved_bytes)?;
    // copy fat tables verbatim
    let mut fat_data = vec![0; bpb.num_fats as usize * bpb.sectors_per_fat as usize * bpb.bytes_per_sector as usize];
    hdi_file.read_exact(&mut fat_data)?;
    //fat_data[0] = 0xF0;
    mo_file.write_all(&fat_data)?;
    // finally write directory and file data
    let mut clusters = vec![];
    hdi_file.read_to_end(&mut clusters)?;
    mo_file.write_all(&clusters)?;
    mo_file.sync_all()?;
    Ok(())
}
