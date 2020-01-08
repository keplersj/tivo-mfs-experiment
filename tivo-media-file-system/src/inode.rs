extern crate chrono;
extern crate nom;
extern crate ovit_util;

use chrono::{DateTime, TimeZone, Utc};
use nom::{
    bytes::streaming::{tag, take},
    error::ErrorKind,
    multi::count,
    number::streaming::{be_u16, be_u32, be_u8},
    Err, IResult,
};
use ovit_util::get_block_from_file;

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

#[derive(Debug, Clone)]
pub struct MFSINodeDataBlock {
    pub sector: u32,
    pub count: u32,
}

impl MFSINodeDataBlock {
    pub fn parse(input: &[u8]) -> IResult<&[u8], MFSINodeDataBlock> {
        let (input, sector) = be_u32(input)?;
        let (input, count) = be_u32(input)?;

        Ok((input, MFSINodeDataBlock { sector, count }))
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
    pub datablocks: Vec<MFSINodeDataBlock>,

    //Added for my conveinence
    pub partition_starting_sector: u32,
    pub sector_in_map: u32,
    pub sector_on_drive: u32,
}

const INODE_DATA_IN_HEADER: u32 = 0x4000_0000;

impl MFSINode {
    pub fn parse(
        input: &[u8],
        partition_starting_sector: u32,
        sector: u32,
    ) -> IResult<&[u8], MFSINode> {
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
        let (input, datablocks) = if flags == INODE_DATA_IN_HEADER {
            (input, vec![])
        } else {
            count(MFSINodeDataBlock::parse, numblocks as usize)(input)?
            // let (input, datablock) = MFSINodeDataBlock::parse(input)?;
            // (input, vec![datablock])
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
                datablocks,

                //Added for my convinence
                partition_starting_sector,
                sector_in_map: sector,
                sector_on_drive: partition_starting_sector + sector,
            },
        ))
    }

    pub fn from_path_at_sector(
        path: &str,
        partition_starting_sector: u32,
        sector: u32,
        is_byte_swapped: bool,
    ) -> Result<MFSINode, String> {
        let inode_bytes = get_block_from_file(
            path,
            u64::from(partition_starting_sector + sector),
            is_byte_swapped,
        )?;

        match MFSINode::parse(&inode_bytes, partition_starting_sector, sector) {
            Ok((_, inode)) => Ok(inode),
            Err(err) => Err(format!("Could not open inode with err {:?}", err)),
        }
    }
}

#[derive(Debug)]
pub struct MFSINodeIter {
    pub source_file_path: String,
    pub partition_starting_sector: u32,
    pub is_source_byte_swapped: bool,

    pub next_inode_sector: u32,
    pub last_inode_sector: u32,
}

impl Iterator for MFSINodeIter {
    type Item = MFSINode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_inode_sector != self.last_inode_sector + 1 {
            let inode = match MFSINode::from_path_at_sector(
                &self.source_file_path,
                self.partition_starting_sector,
                self.next_inode_sector,
                self.is_source_byte_swapped,
            ) {
                Ok(inode) => inode,
                Err(_err) => {
                    println!("{:?}", _err);
                    return None;
                }
            };

            // self.next_inode_sector += 1;
            self.next_inode_sector += 2; //Every inode exists on the drive twice

            Some(inode)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.last_inode_sector as usize - self.next_inode_sector as usize) / 2;
        (size, Some(size))
    }
}

impl ExactSizeIterator for MFSINodeIter {
    // We can easily calculate the remaining number of iterations.
    fn len(&self) -> usize {
        (self.last_inode_sector as usize - self.next_inode_sector as usize) / 2
    }
}
