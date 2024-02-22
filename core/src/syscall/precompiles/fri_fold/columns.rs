use core::borrow::{Borrow, BorrowMut};
use core::mem::size_of;

use sp1_derive::AlignedBorrow;

use crate::air::DEGREE;
use crate::memory::{MemoryReadCols, MemoryReadWriteCols};
use crate::operations::DivExtOperation;

pub(crate) const NUM_FRI_FOLD_COLS: usize = size_of::<FriFoldCols<u8>>();
pub(crate) const X_IDX: usize = 0;
pub(crate) const ALPHA_START_IDX: usize = X_IDX + 1;
pub(crate) const ALPHA_END_IDX: usize = ALPHA_START_IDX + DEGREE - 1;
pub(crate) const Z_START_IDX: usize = ALPHA_END_IDX + 1;
pub(crate) const Z_END_IDX: usize = Z_START_IDX + DEGREE - 1;
pub(crate) const P_AT_Z_START_IDX: usize = Z_END_IDX + 1;
pub(crate) const P_AT_Z_END_IDX: usize = P_AT_Z_START_IDX + DEGREE - 1;
pub(crate) const P_AT_X_IDX: usize = P_AT_Z_END_IDX + 1;
pub(crate) const NUM_INPUT_ELMS: usize = P_AT_X_IDX + 1;

pub(crate) const RO_ADDR_IDX: usize = 0;
pub(crate) const ALPHA_POW_ADDR_IDX: usize = 1;
pub(crate) const NUM_OUTPUT_ELMS: usize = 2;

#[derive(AlignedBorrow)]
#[repr(C)]
pub(crate) struct FriFoldCols<T> {
    pub shard: T,
    pub clk: T,

    // x = input_mem_ptr[0]
    // alpha = input_mem_ptr[1..5]
    // z = input_mem_ptr[5..9]
    // p_at_z = input_mem_ptr[9]
    // p_at_x = input_mem_ptr[10]
    pub input_slice_read_records: [MemoryReadCols<T>; NUM_INPUT_ELMS],
    pub input_slice_ptr: T,

    // ro_addr = output_read_records[0]
    // alpha_pow_addr = output_read_records[1]
    pub output_slice_read_records: [MemoryReadCols<T>; NUM_OUTPUT_ELMS],
    pub output_slice_ptr: T,

    pub ro_rw_records: [MemoryReadWriteCols<T>; DEGREE],
    pub alpha_pow_rw_records: [MemoryReadWriteCols<T>; DEGREE],

    pub(crate) div_ext_op: DivExtOperation<T>,

    pub is_input: T,
    pub is_output: T,
    pub is_real: T,
}
