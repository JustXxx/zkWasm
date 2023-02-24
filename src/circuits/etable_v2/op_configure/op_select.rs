use crate::{
    circuits::{
        cell::*,
        etable_v2::{
            allocator::*, ConstraintBuilder, EventTableCommonConfig, EventTableOpcodeConfig,
            EventTableOpcodeConfigBuilder,
        },
        jtable::{expression::JtableLookupEntryEncode, JumpTableConfig},
        rtable::pow_table_encode,
        utils::{
            bn_to_field, step_status::StepStatus, table_entry::EventTableEntryWithMemoryInfo,
            Context,
        },
    },
    constant, constant_from,
};
use halo2_proofs::{
    arithmetic::FieldExt,
    plonk::{Error, Expression, VirtualCells},
};
use num_bigint::BigUint;
use specs::{
    encode::{frame_table::encode_frame_table_entry, opcode::encode_call},
    etable::EventTableEntry,
    itable::{OpcodeClass, OPCODE_ARG0_SHIFT, OPCODE_ARG1_SHIFT, OPCODE_CLASS_SHIFT},
    mtable::{LocationType, VarType},
    step::StepInfo,
};

pub struct SelectConfig<F: FieldExt> {
    cond: AllocatedU64Cell<F>,
    cond_inv: AllocatedUnlimitedCell<F>,

    val1: AllocatedU64Cell<F>,
    val2: AllocatedU64Cell<F>,
    res: AllocatedU64Cell<F>,
    is_i32: AllocatedBitCell<F>,

    memory_table_lookup_stack_read_cond: AllocatedMemoryTableLookupReadCell<F>,
    memory_table_lookup_stack_read_val2: AllocatedMemoryTableLookupReadCell<F>,
    memory_table_lookup_stack_read_val1: AllocatedMemoryTableLookupReadCell<F>,
    memory_table_lookup_stack_write: AllocatedMemoryTableLookupWriteCell<F>,
}

pub struct SelectConfigBuilder {}

impl<F: FieldExt> EventTableOpcodeConfigBuilder<F> for SelectConfigBuilder {
    fn configure(
        common_config: &EventTableCommonConfig<F>,
        allocator: &mut EventTableCellAllocator<F>,
        constraint_builder: &mut ConstraintBuilder<F>,
    ) -> Box<dyn EventTableOpcodeConfig<F>> {
        let cond = allocator.alloc_u64_cell();
        let cond_inv = allocator.alloc_unlimited_cell();

        let val1 = allocator.alloc_u64_cell();
        let val2 = allocator.alloc_u64_cell();
        let res = allocator.alloc_u64_cell();
        let is_i32 = allocator.alloc_bit_cell();

        constraint_builder.push(
            "select: cond is zero",
            Box::new(move |meta| {
                vec![
                    (constant_from!(1) - cond.u64_cell.expr(meta) * cond_inv.expr(meta))
                        * (res.u64_cell.expr(meta) - val2.u64_cell.expr(meta)),
                ]
            }),
        );

        constraint_builder.push(
            "select: cond is not zero",
            Box::new(move |meta| {
                vec![
                    cond.u64_cell.expr(meta) * (res.u64_cell.expr(meta) - val1.u64_cell.expr(meta)),
                ]
            }),
        );

        let eid = common_config.eid_cell;
        let sp = common_config.sp_cell;

        let memory_table_lookup_stack_read_cond = allocator.alloc_memory_table_lookup_read_cell(
            "op_test stack read",
            constraint_builder,
            eid,
            move |meta| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(1),
            move |meta| constant_from!(1),
            move |meta| cond.u64_cell.expr(meta),
            move |meta| constant_from!(1),
        );

        let memory_table_lookup_stack_read_val2 = allocator.alloc_memory_table_lookup_read_cell(
            "op_test stack read",
            constraint_builder,
            eid,
            move |meta| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(2),
            move |meta| is_i32.expr(meta),
            move |meta| val2.u64_cell.expr(meta),
            move |meta| constant_from!(1),
        );

        let memory_table_lookup_stack_read_val1 = allocator.alloc_memory_table_lookup_read_cell(
            "op_test stack read",
            constraint_builder,
            eid,
            move |meta| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(3),
            move |meta| is_i32.expr(meta),
            move |meta| val1.u64_cell.expr(meta),
            move |meta| constant_from!(1),
        );

        let memory_table_lookup_stack_write = allocator.alloc_memory_table_lookup_write_cell(
            "op_test stack write",
            constraint_builder,
            eid,
            move |meta| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(3),
            move |meta| is_i32.expr(meta),
            move |meta| res.u64_cell.expr(meta),
            move |meta| constant_from!(1),
        );

        Box::new(SelectConfig {
            cond,
            cond_inv,
            val1,
            val2,
            res,
            is_i32,
            memory_table_lookup_stack_read_cond,
            memory_table_lookup_stack_read_val2,
            memory_table_lookup_stack_read_val1,
            memory_table_lookup_stack_write,
        })
    }
}

impl<F: FieldExt> EventTableOpcodeConfig<F> for SelectConfig<F> {
    fn opcode(&self, meta: &mut VirtualCells<'_, F>) -> Expression<F> {
        constant!(bn_to_field(
            &(BigUint::from(OpcodeClass::Select as u64) << OPCODE_CLASS_SHIFT)
        ))
    }

    fn assign(
        &self,
        ctx: &mut Context<'_, F>,
        step: &StepStatus,
        entry: &EventTableEntryWithMemoryInfo,
    ) -> Result<(), Error> {
        match &entry.eentry.step_info {
            StepInfo::Select {
                val1,
                val2,
                cond,
                result,
                vtype,
            } => {
                self.val1.assign(ctx, *val1)?;
                self.val2.assign(ctx, *val2)?;
                self.cond.assign(ctx, *cond)?;
                self.cond_inv
                    .assign(ctx, F::from(*cond).invert().unwrap_or(F::zero()))?;
                self.res.assign(ctx, *result)?;
                self.is_i32.assign_bool(ctx, *vtype == VarType::I32)?;

                self.memory_table_lookup_stack_read_cond.assign(
                    ctx,
                    entry.memory_rw_entires[0].start_eid,
                    step.current.eid,
                    entry.memory_rw_entires[0].end_eid,
                    step.current.sp + 1,
                    LocationType::Stack,
                    true,
                    *cond,
                )?;

                self.memory_table_lookup_stack_read_val2.assign(
                    ctx,
                    entry.memory_rw_entires[1].start_eid,
                    step.current.eid,
                    entry.memory_rw_entires[1].end_eid,
                    step.current.sp + 2,
                    LocationType::Stack,
                    *vtype == VarType::I32,
                    *val2,
                )?;

                self.memory_table_lookup_stack_read_val1.assign(
                    ctx,
                    entry.memory_rw_entires[2].start_eid,
                    step.current.eid,
                    entry.memory_rw_entires[2].end_eid,
                    step.current.sp + 3,
                    LocationType::Stack,
                    *vtype == VarType::I32,
                    *val1,
                )?;

                self.memory_table_lookup_stack_write.assign(
                    ctx,
                    step.current.eid,
                    entry.memory_rw_entires[3].end_eid,
                    step.current.sp + 3,
                    LocationType::Stack,
                    true,
                    *result,
                )?;

                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn sp_diff(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant_from!(2))
    }

    fn mops(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant_from!(1))
    }

    fn memory_writing_ops(&self, entry: &EventTableEntry) -> u32 {
        1
    }
}