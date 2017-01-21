// Copyright (c) IxMilia.  All Rights Reserved.  Licensed under the Apache License, Version 2.0.  See License.txt in the project root for license information.

use std::io::Write;

use ::{
    CodePair,
    CodePairValue,
    Drawing,
    DxfError,
    DxfResult,
    Point,
};

use code_pair_writer::CodePairWriter;
use entities::Entity;
use entity_iter::EntityIter;
use enums::*;
use helper_functions::*;

use itertools::PutBack;

/// A block is a collection of entities.
#[derive(Clone)]
pub struct Block {
    /// The block's handle.
    pub handle: u32,
    /// The block's owner's handle.
    pub owner_handle: u32,
    /// The name of the layer containing the block.
    pub layer: String,
    /// The name of the block.
    pub name: String,
    /// Block-type flags.
    pub flags: i32,
    /// The block's base insertion point.
    pub base_point: Point,
    /// The path name of the XREF.
    pub xref_path_name: String,
    /// The block's description.
    pub description: String,
    /// If the block is in PAPERSPACE or not.
    pub is_in_paperspace: bool,
    /// The entities contained by the block.
    pub entities: Vec<Entity>,
}

// public implementation
impl Block {
    pub fn get_is_anonymous(&self) -> bool {
        self.get_flag(1)
    }
    pub fn set_is_anonymous(&mut self, val: bool) {
        self.set_flag(1, val)
    }
    pub fn has_non_consistent_attribute_definitions(&self) -> bool {
        self.get_flag(2)
    }
    pub fn set_has_non_consistent_attribute_definitions(&mut self, val: bool) {
        self.set_flag(2, val)
    }
    pub fn get_is_xref(&self) -> bool {
        self.get_flag(4)
    }
    pub fn set_is_xref(&mut self, val: bool) {
        self.set_flag(4, val)
    }
    pub fn get_is_xref_overlay(&self) -> bool {
        self.get_flag(8)
    }
    pub fn set_is_xref_overlay(&mut self, val: bool) {
        self.set_flag(8, val)
    }
    pub fn get_is_externally_dependent(&self) -> bool {
        self.get_flag(16)
    }
    pub fn set_is_externally_dependent(&mut self, val: bool) {
        self.set_flag(16, val)
    }
    pub fn get_is_referenced_external_reference(&self) -> bool {
        self.get_flag(32)
    }
    pub fn set_is_referenced_external_reference(&mut self, val: bool) {
        self.set_flag(32, val)
    }
    pub fn get_is_resolved_external_reference(&self) -> bool {
        self.get_flag(64)
    }
    pub fn set_is_resolved_external_reference(&mut self, val: bool) {
        self.set_flag(64, val)
    }
}

impl Default for Block {
    fn default() -> Self {
        Block {
            handle: 0,
            owner_handle: 0,
            layer: String::new(),
            name: String::new(),
            flags: 0,
            base_point: Point::origin(),
            xref_path_name: String::new(),
            description: String::new(),
            is_in_paperspace: false,
            entities: vec![],
        }
    }
}

// public but should be internal implementation
impl Block {
    #[doc(hidden)]
    pub fn read_block<I>(drawing: &mut Drawing, iter: &mut PutBack<I>) -> DxfResult<()>
        where I: Iterator<Item = DxfResult<CodePair>> {

        // match code pair:
        //   0/ENDBLK -> swallow code pairs and return
        //   0/* -> read entity and add to collection
        //   */* -> apply to block
        let mut current = Block::default();
        loop {
            match iter.next() {
                Some(Ok(pair)) => {
                    match pair {
                        CodePair { code: 0, value: CodePairValue::Str(ref s) } if s == "ENDBLK" => {
                            // swallow all non-0 code pairs
                            loop {
                                match iter.next() {
                                    Some(Ok(pair @ CodePair { code: 0, .. })) => {
                                        // done reading ENDBLK
                                        iter.put_back(Ok(pair));
                                        break;
                                    },
                                    Some(Ok(_)) => (), // swallow this
                                    Some(Err(e)) => return Err(e),
                                    None => return Err(DxfError::UnexpectedEndOfInput),
                                }
                            }

                            drawing.blocks.push(current);
                            break;
                        },
                        CodePair { code: 0, .. } => {
                            // should be an entity
                            iter.put_back(Ok(pair));
                            let mut iter = EntityIter { iter: iter };
                            try!(iter.read_entities_into_vec(&mut current.entities));
                        },
                        _ => {
                            // specific to the BLOCK
                            match pair.code {
                                1 => current.xref_path_name = try!(pair.value.assert_string()),
                                2 => current.name = try!(pair.value.assert_string()),
                                3 => (), // another instance of the name
                                4 => current.description = try!(pair.value.assert_string()),
                                5 => current.handle = try!(as_u32(try!(pair.value.assert_string()))),
                                8 => current.layer = try!(pair.value.assert_string()),
                                10 => current.base_point.x = try!(pair.value.assert_f64()),
                                20 => current.base_point.y = try!(pair.value.assert_f64()),
                                30 => current.base_point.z = try!(pair.value.assert_f64()),
                                67 => current.is_in_paperspace = as_bool(try!(pair.value.assert_i16())),
                                70 => current.flags = try!(pair.value.assert_i16()) as i32,
                                330 => current.owner_handle = try!(as_u32(try!(pair.value.assert_string()))),
                                _ => (), // unsupported code pair
                            }
                        },
                    }
                },
                Some(Err(e)) => return Err(e),
                None => return Err(DxfError::UnexpectedEndOfInput),
            }
        }

        Ok(())
    }
    #[doc(hidden)]
    pub fn write<T>(&self, version: &AcadVersion, write_handles: bool, writer: &mut CodePairWriter<T>) -> DxfResult<()>
        where T: Write {

        try!(writer.write_code_pair(&CodePair::new_str(0, "BLOCK")));
        if write_handles && self.handle != 0 {
            try!(writer.write_code_pair(&CodePair::new_string(5, &as_handle(self.handle))));
        }

        // TODO: XData
        if version >= &AcadVersion::R13 {
            if self.owner_handle != 0 {
                try!(writer.write_code_pair(&CodePair::new_string(330, &as_handle(self.owner_handle))));
            }

            try!(writer.write_code_pair(&CodePair::new_str(100, "AcDbEntity")));
        }

        if self.is_in_paperspace {
            try!(writer.write_code_pair(&CodePair::new_i16(67, as_i16(self.is_in_paperspace))));
        }

        try!(writer.write_code_pair(&CodePair::new_string(8, &self.layer)));
        if version >= &AcadVersion::R13 {
            try!(writer.write_code_pair(&CodePair::new_str(100, "AcDbBlockBegin")));
        }

        try!(writer.write_code_pair(&CodePair::new_string(2, &self.name)));
        try!(writer.write_code_pair(&CodePair::new_i16(70, self.flags as i16)));
        try!(writer.write_code_pair(&CodePair::new_f64(10, self.base_point.x)));
        try!(writer.write_code_pair(&CodePair::new_f64(20, self.base_point.y)));
        try!(writer.write_code_pair(&CodePair::new_f64(30, self.base_point.z)));
        if version >= &AcadVersion::R12 {
            try!(writer.write_code_pair(&CodePair::new_string(3, &self.name)));
        }

        try!(writer.write_code_pair(&CodePair::new_string(1, &self.xref_path_name)));
        if !self.description.is_empty() {
            try!(writer.write_code_pair(&CodePair::new_string(4, &self.description)));
        }

        for e in &self.entities {
            try!(e.write(version, false, writer)); // entities in blocks never have handles
        }

        try!(writer.write_code_pair(&CodePair::new_str(0, "ENDBLK")));
        if write_handles && self.handle != 0 {
            try!(writer.write_code_pair(&CodePair::new_string(5, &as_handle(self.handle))));
        }

        // TODO: XData
        // TODO: extension data groups
        if version >= &AcadVersion::R2000 && self.owner_handle != 0 {
            try!(writer.write_code_pair(&CodePair::new_string(330, &as_handle(self.owner_handle))));
        }

        if version >= &AcadVersion::R13 {
            try!(writer.write_code_pair(&CodePair::new_str(100, "AcDbEntity")));
        }

        if self.is_in_paperspace {
            try!(writer.write_code_pair(&CodePair::new_i16(67, as_i16(self.is_in_paperspace))));
        }

        try!(writer.write_code_pair(&CodePair::new_string(8, &self.layer)));
        if version >= &AcadVersion::R13 {
            try!(writer.write_code_pair(&CodePair::new_str(100, "AcDbBlockEnd")));
        }

        Ok(())
    }
}

// private implementation
impl Block {
    fn get_flag(&self, mask: i32) -> bool {
        self.flags & mask != 0
    }
    fn set_flag(&mut self, mask: i32, val: bool) {
        if val {
            self.flags |= mask;
        }
        else {
            self.flags &= !mask;
        }
    }
}
