extern crate positioned_io;

use std::convert::TryInto;

use std::fs::File;

use std::io::prelude::*;

use std::ops::RangeInclusive;

use std::vec::Vec;

use positioned_io::ReadAt;

pub const TIVO_BOOT_MAGIC: u16 = 0x1492;
pub const TIVO_BOOT_AMIGC: u16 = 0x9214;
pub const APM_BLOCK_SIZE: usize = 512;
pub const MFS32_HEADER_MAGIC: u32 = 0xABBA_FEED;
pub const MFS64_HEADER_MAGIC: u32 = 0xEBBA_FEED;

pub fn get_string_from_bytes_range(
    bytes: &[u8],
    range: RangeInclusive<usize>,
) -> Result<String, String> {
    match String::from_utf8(match bytes.get(range) {
        Some(vec) => vec.to_vec(),
        _ => {
            return Err("Could not get bytes".to_string());
        }
    }) {
        Ok(string) => Ok(string.to_string()),
        Err(err) => Err(format!("Could not convert bytes to string: {:#X?}", err)),
    }
}

pub fn get_u32_from_bytes_range(bytes: &[u8], range: RangeInclusive<usize>) -> u32 {
    u32::from_be_bytes(
        bytes
            .get(range)
            .expect("Could not get partitions total from partition entry")
            .try_into()
            .unwrap(),
    )
}

pub fn correct_byte_order(raw_buffer: &[u8], is_byte_swapped: bool) -> Vec<u8> {
    raw_buffer
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes(chunk[0..2].try_into().unwrap()))
        .map(|byte| {
            if is_byte_swapped {
                byte
            } else {
                byte.swap_bytes()
            }
        })
        .flat_map(|byte| -> Vec<u8> { byte.to_ne_bytes().to_vec() })
        .collect()
}

pub fn get_block_from_drive(file: &File, location: u64) -> Result<Vec<u8>, String> {
    get_blocks_from_drive(file, location, 1)
}

pub fn get_blocks_from_drive(file: &File, location: u64, count: usize) -> Result<Vec<u8>, String> {
    let mut buffer = vec![0; APM_BLOCK_SIZE * count];

    match file.read_at(location * APM_BLOCK_SIZE as u64, &mut buffer) {
        Ok(_) => Ok(buffer),
        Err(_) => Err(format!(
            "Could not read block from file at location {}",
            location
        )),
    }
}

#[derive(Debug)]
pub struct Partition {
    pub signature: String,
    pub partitions_total: u32,
    pub starting_sector: u32,
    pub sector_size: u32,
    pub name: String,
    pub r#type: String,
    pub starting_data_sector: u32,
    pub data_sectors: u32,
    pub status: String,
}

impl Partition {
    pub fn new(bytes: Vec<u8>) -> Result<Partition, &'static str> {
        let signature =
            get_string_from_bytes_range(&bytes, 0..=1).expect("Could not get signature from bytes");

        if signature != "PM" {
            return Err("Invalid signature in sector");
        }

        let partitions_total = get_u32_from_bytes_range(&bytes, 4..=7);

        let starting_sector = get_u32_from_bytes_range(&bytes, 8..=11);

        let sector_size = get_u32_from_bytes_range(&bytes, 12..=15);

        let name = get_string_from_bytes_range(&bytes, 16..=47)
            .expect("Could not get name from bytes")
            .trim_matches(char::from(0))
            .to_string();

        let r#type = get_string_from_bytes_range(&bytes, 48..=79)
            .expect("Could not get type from bytes")
            .trim_matches(char::from(0))
            .to_string();

        let starting_data_sector = get_u32_from_bytes_range(&bytes, 80..=83);

        let data_sectors = get_u32_from_bytes_range(&bytes, 84..=87);

        let status = format!("{:#X}", get_u32_from_bytes_range(&bytes, 88..=91));

        Ok(Partition {
            signature,
            partitions_total,
            starting_sector,
            sector_size,
            name,
            r#type,
            starting_data_sector,
            data_sectors,
            status,
        })
    }
}

#[derive(Debug)]
pub struct ApplePartitionMap {
    pub partitions: Vec<Partition>,
}

#[derive(Debug)]
pub struct MFSVolumeHeader {
    pub state: u32,
    pub magic: String,
    pub checksum: u32,
    pub root_fsid: u32,
    pub firstpartsize: u32,
    pub partitionlist: String,
    pub total_sectors: u32,
    pub zonemap_ptr: u32,
    pub backup_zonemap_ptr: u32,
    pub zonemap_size: u32,
    pub next_fsid: u32,
}

impl MFSVolumeHeader {
    pub fn from_bytes(block: &[u8]) -> MFSVolumeHeader {
        MFSVolumeHeader {
            state: get_u32_from_bytes_range(block, 0..=3),
            magic: format!("{:X}", get_u32_from_bytes_range(block, 4..=7)),
            checksum: get_u32_from_bytes_range(block, 8..=11),
            root_fsid: get_u32_from_bytes_range(block, 16..=19),
            firstpartsize: get_u32_from_bytes_range(block, 20..=23),
            partitionlist: get_string_from_bytes_range(block, 36..=163)
                .expect("Could not get device list from bytes")
                .trim_matches(char::from(0))
                .to_string(),
            total_sectors: get_u32_from_bytes_range(block, 164..=167),
            zonemap_ptr: get_u32_from_bytes_range(block, 196..=199),
            backup_zonemap_ptr: get_u32_from_bytes_range(block, 200..=203),
            zonemap_size: get_u32_from_bytes_range(block, 204..=207),
            next_fsid: get_u32_from_bytes_range(block, 216..=219),
        }
    }
}

#[derive(Debug)]
pub struct MFSZoneMap {
    pub sector: u32,
    pub backup_sector: u32,
    pub zonemap_size: u32,
    pub next_zonemap_ptr: u32,
    pub backup_next_zonemap_ptr: u32,
    pub next_zonemap_size: u32,
    pub r#type: u32,
    pub crc: u32,
    pub zone_start: u32,
    pub next_zone_ptr1: u32,
    pub zone_size: u32,
    pub per_chunk: u32,
    pub buddy_size: u32,
}

impl MFSZoneMap {
    pub fn from_bytes(block: &[u8]) -> MFSZoneMap {
        MFSZoneMap {
            sector: get_u32_from_bytes_range(block, 0..=3),
            backup_sector: get_u32_from_bytes_range(block, 4..=7),
            zonemap_size: get_u32_from_bytes_range(block, 8..=11),
            next_zonemap_ptr: get_u32_from_bytes_range(block, 12..=15),
            backup_next_zonemap_ptr: get_u32_from_bytes_range(block, 16..=19),
            next_zonemap_size: get_u32_from_bytes_range(block, 20..=23),
            r#type: get_u32_from_bytes_range(block, 28..=31),
            crc: get_u32_from_bytes_range(block, 36..=39),
            zone_start: get_u32_from_bytes_range(block, 40..=43),
            next_zone_ptr1: get_u32_from_bytes_range(block, 44..=47),
            zone_size: get_u32_from_bytes_range(block, 48..=51),
            per_chunk: get_u32_from_bytes_range(block, 52..=55),
            buddy_size: get_u32_from_bytes_range(block, 60..=63),
        }
    }
}

#[derive(Debug)]
pub struct TivoDrive {
    pub partition_map: ApplePartitionMap,
    pub volume_header: MFSVolumeHeader,
    pub zones: Vec<MFSZoneMap>,
}

impl TivoDrive {
    pub fn from_disk_image(path: &str) -> Result<TivoDrive, &'static str> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => {
                return Err("Couldn't open image");
            }
        };

        let mut buffer = [0; 2];
        match file.read_exact(&mut buffer) {
            Ok(_) => {}
            Err(_) => {
                return Err("Could not read first two bytes from file");
            }
        };

        let is_byte_swapped = match u16::from_be_bytes(buffer[0..2].try_into().unwrap()) {
            TIVO_BOOT_MAGIC => false,
            TIVO_BOOT_AMIGC => true,
            _ => {
                return Err("Not a TiVo Drive");
            }
        };

        // The first block on a TiVo drive contain special TiVo magic,
        //  we're not worried about this for reconstructing the partition map.
        //  The partition entry describing the partition map should be in the second block (offet: 512)
        let driver_descriptor_buffer = match get_block_from_drive(&file, 1) {
            Ok(buffer) => correct_byte_order(&buffer, is_byte_swapped),
            Err(_) => {
                return Err("Could not read block containing partition map");
            }
        };

        let driver_descriptor_map = Partition::new(driver_descriptor_buffer)
            .expect("Could not reconstruct Driver Descriptor Map");

        let mut partitions = vec![driver_descriptor_map];

        for offset in 2..=partitions.get(0).unwrap().partitions_total {
            let partition_buffer = match get_block_from_drive(&file, u64::from(offset)) {
                Ok(buffer) => correct_byte_order(&buffer, is_byte_swapped),
                Err(_) => {
                    return Err("Could not read block containing partition map");
                }
            };

            match Partition::new(partition_buffer) {
                Ok(partition) => {
                    partitions.push(partition);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        let partition_map = ApplePartitionMap { partitions };

        let app_region = partition_map
            .partitions
            .iter()
            .find(|partition| partition.r#type == "MFS")
            .unwrap();

        let app_region_block = correct_byte_order(
            &get_block_from_drive(&file, u64::from(app_region.starting_sector)).unwrap(),
            is_byte_swapped,
        );

        let volume_header = MFSVolumeHeader::from_bytes(&app_region_block);

        let first_zonemap_block = correct_byte_order(
            &get_block_from_drive(
                &file,
                u64::from(app_region.starting_sector + volume_header.zonemap_ptr),
            )
            .unwrap(),
            is_byte_swapped,
        );

        let first_zonemap = MFSZoneMap::from_bytes(&first_zonemap_block);

        let mut zones = vec![first_zonemap];

        let mut next_zone_ptr = zones[0].next_zonemap_ptr;
        let mut next_zone_size = zones[0].next_zonemap_size;

        while next_zone_ptr != 0 {
            let zonemap_bytes = correct_byte_order(
                &match get_blocks_from_drive(
                    &file,
                    u64::from(app_region.starting_sector + next_zone_ptr),
                    next_zone_size as usize,
                ) {
                    Ok(blocks) => blocks,
                    Err(_) => {
                        println!(
                            "Couldn't load zonemap blocks at ptr and size: {}, {}",
                            next_zone_ptr, next_zone_size
                        );
                        println!("Lazilly continuing");
                        break;
                    }
                },
                is_byte_swapped,
            );

            let zonemap = MFSZoneMap::from_bytes(&zonemap_bytes);

            next_zone_ptr = zonemap.next_zonemap_ptr;
            next_zone_size = zonemap.next_zonemap_size;

            zones.push(zonemap);
        }

        Ok(TivoDrive {
            partition_map,
            volume_header,
            zones,
        })
    }
}