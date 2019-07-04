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
    let mut buffer = vec![0; APM_BLOCK_SIZE];

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
pub struct TivoDrive {
    pub partition_map: ApplePartitionMap,
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

        Ok(TivoDrive {
            partition_map: ApplePartitionMap { partitions },
        })
    }
}
