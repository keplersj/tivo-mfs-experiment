extern crate chrono;
extern crate nom;

use chrono::{DateTime, TimeZone, Utc};
use nom::{
    bytes::streaming::{tag, take},
    error::ErrorKind,
    number::streaming::{be_u16, be_u32, be_u8},
    Err, IResult,
};

fn string(input: &[u8]) -> IResult<&[u8], String> {
    let (input, str_bytes) = take(128 as usize)(input)?;
    match String::from_utf8(str_bytes.to_vec()) {
        Ok(string) => Ok((input, string.trim_matches(char::from(0)).to_string())),
        Err(_) => Err(Err::Error((input, ErrorKind::ParseTo))),
    }
}

#[derive(Debug, PartialEq)]
pub struct MFSVolumeHeader {
    pub state: u32,
    pub checksum: u32,
    pub root_fsid: u32,
    pub firstpartsize: u32,
    pub partitionlist: String,
    pub total_sectors: u32,
    pub next_zonemap_sector: u32,
    pub next_zonemap_backup_sector: u32,
    pub next_zonemap_partition_size: u32,
    pub next_fsid: u32,
}

impl MFSVolumeHeader {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSVolumeHeader> {
        let (input, state) = be_u32(input)?;
        let (input, _) = tag([0xAB, 0xBA, 0xFE, 0xED])(input)?;
        let (input, checksum) = be_u32(input)?;
        let (input, _) = take(4 as usize)(input)?;
        let (input, root_fsid) = be_u32(input)?;
        let (input, _) = take(4 as usize)(input)?;
        let (input, firstpartsize) = be_u32(input)?;
        let (input, _) = take(4 as usize)(input)?;
        let (input, _) = take(4 as usize)(input)?;
        let (input, partitionlist) = string(input)?;
        let (input, total_sectors) = be_u32(input)?;
        let (input, _) = take(4 as usize)(input)?;
        let (input, _logstart) = be_u32(input)?;
        let (input, _lognsectors) = be_u32(input)?;
        let (input, _volhdrlogstamp) = be_u32(input)?;
        let (input, _unkstart) = be_u32(input)?;
        let (input, _unksectors) = be_u32(input)?;
        let (input, _unkstamp) = be_u32(input)?;
        let (input, next_zonemap_sector) = be_u32(input)?;
        let (input, next_zonemap_backup_sector) = be_u32(input)?;
        let (input, _next_zonemap_sector_length) = be_u32(input)?;
        let (input, next_zonemap_partition_size) = be_u32(input)?;
        let (input, _next_zonemap_min_allocation) = be_u32(input)?;
        let (input, next_fsid) = be_u32(input)?;
        let (input, _bootcycles) = be_u32(input)?;
        let (input, _bootsecs) = be_u32(input)?;
        let (input, _) = take(4 as usize)(input)?;

        Ok((
            input,
            MFSVolumeHeader {
                state,
                checksum,
                root_fsid,
                firstpartsize,
                partitionlist,
                total_sectors,
                next_zonemap_sector,
                next_zonemap_backup_sector,
                next_zonemap_partition_size,
                next_fsid,
            },
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MFSZoneType {
    INode = 0,
    Application = 1,
    Media = 2,
    Max = 3,
}

impl MFSZoneType {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSZoneType> {
        let (input, n) = be_u32(input)?;
        match n {
            0 => Ok((input, MFSZoneType::INode)),
            1 => Ok((input, MFSZoneType::Application)),
            2 => Ok((input, MFSZoneType::Media)),
            3 => Ok((input, MFSZoneType::Max)),
            _ => Err(Err::Error((input, ErrorKind::NoneOf))),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MFSZoneMap {
    pub sector: u32,
    pub backup_sector: u32,
    pub zonemap_size: u32,
    pub next_zonemap_ptr: u32,
    pub backup_next_zonemap_ptr: u32,
    pub next_zonemap_size: u32,
    pub next_zonemap_partition_size: u32,
    pub next_zonemap_min_allocation: u32,
    pub logstamp: u32,
    pub r#type: MFSZoneType,
    pub checksum: u32,
    pub first_sector: u32,
    pub last_sector: u32,
    pub size: u32,
    pub min_allocations: u32,
    pub free_space: u32,
    pub bitmap_num: u32,
}

impl MFSZoneMap {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSZoneMap> {
        let (input, sector) = be_u32(input)?;
        let (input, backup_sector) = be_u32(input)?;
        let (input, zonemap_size) = be_u32(input)?;
        let (input, next_zonemap_ptr) = be_u32(input)?;
        let (input, backup_next_zonemap_ptr) = be_u32(input)?;
        let (input, next_zonemap_size) = be_u32(input)?;
        let (input, next_zonemap_partition_size) = be_u32(input)?;
        let (input, next_zonemap_min_allocation) = be_u32(input)?;
        let (input, r#type) = MFSZoneType::parse(input)?;
        let (input, logstamp) = be_u32(input)?;
        let (input, checksum) = be_u32(input)?;
        let (input, first_sector) = be_u32(input)?;
        let (input, last_sector) = be_u32(input)?;
        let (input, size) = be_u32(input)?;
        let (input, min_allocations) = be_u32(input)?;
        let (input, free_space) = be_u32(input)?;
        let (input, _) = tag([0, 0, 0, 0])(input)?;
        let (input, bitmap_num) = be_u32(input)?;

        Ok((
            input,
            MFSZoneMap {
                sector,
                backup_sector,
                zonemap_size,
                next_zonemap_ptr,
                backup_next_zonemap_ptr,
                next_zonemap_size,
                next_zonemap_partition_size,
                next_zonemap_min_allocation,
                r#type,
                logstamp,
                checksum,
                first_sector,
                last_sector,
                size,
                min_allocations,
                free_space,
                bitmap_num,
            },
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MFSINodeType {
    Node = 0,
    File = 1,
    Stream = 2,
    Dir = 4,
    Db = 8,
}

impl MFSINodeType {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSINodeType> {
        let (input, n) = be_u8(input)?;
        match n {
            0 => Ok((input, MFSINodeType::Node)),
            1 => Ok((input, MFSINodeType::File)),
            2 => Ok((input, MFSINodeType::Stream)),
            4 => Ok((input, MFSINodeType::Dir)),
            8 => Ok((input, MFSINodeType::Db)),
            _ => Err(Err::Error((input, ErrorKind::NoneOf))),
        }
    }
}

#[derive(Debug)]
pub struct MFSINode {
    pub fsid: u32,
    pub refcount: u32,
    pub bootcycles: u32,
    pub bootsecs: u32,
    pub inode: u32,
    pub size: u32,
    pub blocksize: u32,
    pub blockused: u32,
    pub last_modified: DateTime<Utc>,
    pub r#type: MFSINodeType,
    pub zone: u8,
    pub checksum: u32,
    pub flags: u32,
    pub data: Vec<u8>,
    pub numblocks: u32,
    pub data_block_sector: u32,
    pub data_block_count: u32,
}

const INODE_DATA_IN_HEADER: u32 = 0x4000_0000;

impl MFSINode {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSINode> {
        let (input, fsid) = be_u32(input)?;
        let (input, refcount) = be_u32(input)?;
        let (input, bootcycles) = be_u32(input)?;
        let (input, bootsecs) = be_u32(input)?;
        let (input, inode) = be_u32(input)?; // Should be (sectornum - 1122) / 2
        let (input, _) = take(4 as usize)(input)?;
        let (input, size) = be_u32(input)?;
        let (input, blocksize) = be_u32(input)?;
        let (input, blockused) = be_u32(input)?;
        let (input, last_modified) = be_u32(input)?;
        let (input, r#type) = MFSINodeType::parse(input)?;
        let (input, zone) = be_u8(input)?;
        let (input, _pad) = be_u16(input)?;
        let (input, _sig) = tag([0x91, 0x23, 0x1e, 0xbc])(input)?;
        let (input, checksum) = be_u32(input)?;
        let (input, flags) = be_u32(input)?;
        let (input, data) = if flags == INODE_DATA_IN_HEADER {
            let data = input.to_vec();
            let input: &[u8] = &[];
            (input, data)
        } else {
            (input, vec![])
        };
        let (input, numblocks) = if flags == INODE_DATA_IN_HEADER {
            (input, 0)
        } else {
            be_u32(input)?
        };
        let (input, data_block_sector) = if flags == INODE_DATA_IN_HEADER {
            (input, 0)
        } else {
            be_u32(input)?
        };
        let (input, data_block_count) = if flags == INODE_DATA_IN_HEADER {
            (input, 0)
        } else {
            be_u32(input)?
        };

        Ok((
            input,
            MFSINode {
                fsid,
                refcount,
                bootcycles,
                bootsecs,
                inode,
                size,
                blocksize,
                blockused,
                last_modified: Utc.timestamp(i64::from(last_modified), 0),
                r#type,
                zone,
                checksum,
                flags,
                data,
                numblocks,
                data_block_sector,
                data_block_count,
            },
        ))
    }
}