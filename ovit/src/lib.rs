extern crate apple_partition_map;
extern crate rayon;
extern crate tivo_media_file_system;

use apple_partition_map::ApplePartitionMap;
use rayon::prelude::*;
use std::convert::TryInto;
use std::fs::File;
use std::io::prelude::*;
use tivo_media_file_system::{MFSINode, MFSVolumeHeader, MFSZoneMap};

pub const TIVO_BOOT_MAGIC: u16 = 0x1492;
pub const TIVO_BOOT_AMIGC: u16 = 0x9214;

fn fsid_hash(fsid: u64, size: u64) -> u64 {
    // Prime number used in hash for finding base inode of fsid. (from mfstools)
    const FSID_HASH: u64 = 0x106d9;

    (fsid * FSID_HASH) & (size)
}

fn sector_for_inode(inode: u64) -> u64 {
    (2 * inode) + 1122
}

#[derive(Debug)]
pub struct TivoDrive {
    pub source_file: File,
    pub partition_map: ApplePartitionMap,
    pub volume_header: MFSVolumeHeader,
    pub zonemap: MFSZoneMap,
    pub is_byte_swapped: bool,
}

impl TivoDrive {
    fn check_byte_order(file: &mut File) -> Result<bool, String> {
        let mut buffer = [0; 2];
        match file.read_exact(&mut buffer) {
            Ok(_) => {}
            Err(_) => {
                return Err("Could not read first two bytes from file".to_string());
            }
        };

        match u16::from_be_bytes(buffer[0..2].try_into().unwrap()) {
            TIVO_BOOT_MAGIC => Ok(false),
            TIVO_BOOT_AMIGC => Ok(true),
            _ => Err("Not a TiVo Drive".to_string()),
        }
    }

    pub fn from_disk_image(path: &str) -> Result<TivoDrive, String> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => {
                return Err("Couldn't open drive".to_string());
            }
        };

        let is_byte_swapped = TivoDrive::check_byte_order(&mut file)?;

        let partition_map = ApplePartitionMap::read_from_file(&mut file, is_byte_swapped)?;

        let app_region = partition_map
            .partitions
            .iter()
            .find(|partition| partition.r#type == "MFS")
            .unwrap();

        let volume_header =
            MFSVolumeHeader::from_partition(app_region, &mut file, is_byte_swapped)?;

        let zonemap = MFSZoneMap::new(
            path,
            u64::from(app_region.starting_sector),
            volume_header.next_zonemap_sector,
            volume_header.next_zonemap_backup_sector,
            volume_header.next_zonemap_partition_size as usize,
            is_byte_swapped,
        )?;

        Ok(TivoDrive {
            source_file: file,
            partition_map,
            volume_header,
            zonemap,
            is_byte_swapped,
        })
    }

    pub fn get_inode_from_fsid(&mut self, fsid: u32) -> Result<MFSINode, String> {
        let inode_iter = self.zonemap.inode_iter().unwrap();

        let inode_count: u64 = (inode_iter.len()).try_into().unwrap();

        let inode: u64 = fsid_hash(u64::from(fsid), inode_count);
        let sector = sector_for_inode(inode);

        let first_inode = MFSINode::from_file_at_sector(
            &mut self.source_file,
            self.zonemap.partition_starting_sector,
            sector,
            self.is_byte_swapped,
        )?;
        let first_fsid = first_inode.fsid;

        if first_fsid == fsid {
            return Ok(first_inode);
        };

        let inode_id_base = first_inode.inode;
        let mut current_inode_id = inode;
        let mut current_inode = first_inode;

        println!("Couldn't find INode for FSID {}. Looking for it.", fsid);

        while current_inode.fsid != fsid
            && current_inode.flags == 0x8000_0000
            && ((current_inode_id + 1) % (inode_count)) != u64::from(inode_id_base)
        {
            current_inode_id += 1;
            current_inode = MFSINode::from_file_at_sector(
                &mut self.source_file,
                self.zonemap.partition_starting_sector,
                sector_for_inode(current_inode_id),
                self.is_byte_swapped,
            )?;
        }

        if current_inode.fsid == fsid {
            return Ok(current_inode);
        };

        Err(format!("Could not get INode for FSID {}", fsid))
    }
}
