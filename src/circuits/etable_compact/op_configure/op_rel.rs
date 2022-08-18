use super::*;
use crate::{
    circuits::utils::{bn_to_field, Context},
    constant,
};
use halo2_proofs::{
    arithmetic::FieldExt,
    plonk::{Error, Expression, VirtualCells},
};
use specs::itable::{RelOp, OPCODE_ARG1_SHIFT};
use specs::mtable::VarType;
use specs::step::StepInfo;
use specs::{
    etable::EventTableEntry,
    itable::{OpcodeClass, OPCODE_ARG0_SHIFT, OPCODE_CLASS_SHIFT},
};

pub struct RelConfig {
    // vtype

    // TODO: we can use `1 - is_eight_bytes` to replace `is_four_bytes`
    is_four_bytes: BitCell,
    is_eight_bytes: BitCell,
    is_sign: BitCell,

    lhs: U64Cell,
    rhs: U64Cell,
    diff: U64Cell,

    diff_inv: UnlimitedCell,
    res_is_eq: BitCell,
    res_is_lt: BitCell,
    res_is_gt: BitCell,
    res: UnlimitedCell,

    lhs_leading_bit: BitCell,
    rhs_leading_bit: BitCell,
    lhs_rem_value: CommonRangeCell,
    rhs_rem_value: CommonRangeCell,

    op_is_eq: BitCell,
    op_is_ne: BitCell,
    op_is_lt: BitCell,
    op_is_gt: BitCell,
    op_is_le: BitCell,
    op_is_ge: BitCell,
    op_is_sign: BitCell,

    lookup_stack_read_lhs: MTableLookupCell,
    lookup_stack_read_rhs: MTableLookupCell,
    lookup_stack_write_res: MTableLookupCell,
}

pub struct RelConfigBuilder {}

impl<F: FieldExt> EventTableOpcodeConfigBuilder<F> for RelConfigBuilder {
    fn configure(
        common: &mut EventTableCellAllocator<F>,
        constraint_builder: &mut ConstraintBuilder<F>,
    ) -> Box<dyn EventTableOpcodeConfig<F>> {
        let diff_inv = common.alloc_unlimited_value();
        let res_is_eq = common.alloc_bit_value();
        let res_is_lt = common.alloc_bit_value();
        let res_is_gt = common.alloc_bit_value();
        let res = common.alloc_unlimited_value();

        let lhs = common.alloc_u64();
        let rhs = common.alloc_u64();
        let diff = common.alloc_u64();

        let lhs_leading_bit = common.alloc_bit_value();
        let rhs_leading_bit = common.alloc_bit_value();
        let lhs_rem_value = common.alloc_common_range_value();
        let rhs_rem_value = common.alloc_common_range_value();

        let op_is_eq = common.alloc_bit_value();
        let op_is_ne = common.alloc_bit_value();
        let op_is_lt = common.alloc_bit_value();
        let op_is_gt = common.alloc_bit_value();
        let op_is_le = common.alloc_bit_value();
        let op_is_ge = common.alloc_bit_value();
        let op_is_sign = common.alloc_bit_value();

        let is_four_bytes = common.alloc_bit_value();
        let is_eight_bytes = common.alloc_bit_value();
        let is_sign = common.alloc_bit_value();

        let lookup_stack_read_lhs = common.alloc_mtable_lookup();
        let lookup_stack_read_rhs = common.alloc_mtable_lookup();
        let lookup_stack_write_res = common.alloc_mtable_lookup();

        constraint_builder.push(
            "compare diff",
            Box::new(move |meta| {
                vec![
                    (lhs.expr(meta) + res_is_lt.expr(meta) * diff.expr(meta)
                        - res_is_gt.expr(meta) * diff.expr(meta)
                        - rhs.expr(meta)),
                    (res_is_gt.expr(meta) + res_is_lt.expr(meta) + res_is_eq.expr(meta)
                        - constant_from!(1)),
                    (diff.expr(meta) * res_is_eq.expr(meta)),
                    (diff.expr(meta) * diff_inv.expr(meta) + res_is_eq.expr(meta)
                        - constant_from!(1)),
                ]
            }),
        );

        constraint_builder.push(
            "compare op",
            Box::new(move |meta| {
                vec![
                    (op_is_eq.expr(meta)
                        + op_is_ne.expr(meta)
                        + op_is_lt.expr(meta)
                        + op_is_gt.expr(meta)
                        + op_is_le.expr(meta)
                        + op_is_ge.expr(meta)
                        - constant_from!(1)),
                ]
            }),
        );

        constraint_builder.push(
            "compare bytes",
            Box::new(move |meta| {
                vec![is_four_bytes.expr(meta) + is_eight_bytes.expr(meta) - constant_from!(1)]
            }),
        );

        constraint_builder.push(
            "compare leading bit",
            Box::new(move |meta| {
                vec![
                    lhs_leading_bit.expr(meta) * constant_from!(8) + lhs_rem_value.expr(meta)
                        - (is_four_bytes.expr(meta) * lhs.u4_expr(meta, 7)
                            + is_eight_bytes.expr(meta) * lhs.u4_expr(meta, 15))
                            * op_is_sign.expr(meta),
                    rhs_leading_bit.expr(meta) * constant_from!(8) + rhs_rem_value.expr(meta)
                        - (is_four_bytes.expr(meta) * rhs.u4_expr(meta, 7)
                            + is_eight_bytes.expr(meta) * rhs.u4_expr(meta, 15))
                            * op_is_sign.expr(meta),
                ]
            }),
        );

        constraint_builder.push(
            "compare op res",
            Box::new(move |meta| {
                let l_pos_r_pos = (constant_from!(1) - lhs_leading_bit.expr(meta))
                    * (constant_from!(1) - rhs_leading_bit.expr(meta));
                let l_pos_r_neg =
                    (constant_from!(1) - lhs_leading_bit.expr(meta)) * rhs_leading_bit.expr(meta);
                let l_neg_r_pos =
                    lhs_leading_bit.expr(meta) * (constant_from!(1) - rhs_leading_bit.expr(meta));
                let l_neg_r_neg = lhs_leading_bit.expr(meta) * rhs_leading_bit.expr(meta);
                vec![
                    op_is_eq.expr(meta) * (res.expr(meta) - res_is_eq.expr(meta)),
                    op_is_ne.expr(meta)
                        * (res.expr(meta) - constant_from!(1) + res_is_eq.expr(meta)),
                    op_is_lt.expr(meta)
                        * (res.expr(meta)
                            - l_neg_r_pos.clone()
                            - l_pos_r_pos.clone() * res_is_lt.expr(meta)
                            - l_neg_r_neg.clone() * res_is_gt.expr(meta)),
                    op_is_le.expr(meta)
                        * (res.expr(meta)
                            - l_neg_r_pos.clone()
                            - l_pos_r_pos.clone() * res_is_lt.expr(meta)
                            - l_neg_r_neg.clone() * res_is_gt.expr(meta)
                            - res_is_eq.expr(meta)),
                    op_is_gt.expr(meta)
                        * (res.expr(meta)
                            - l_pos_r_neg.clone()
                            - l_pos_r_pos.clone() * res_is_gt.expr(meta)
                            - l_neg_r_neg.clone() * res_is_lt.expr(meta)),
                    op_is_ge.expr(meta)
                        * (res.expr(meta)
                            - l_pos_r_neg.clone()
                            - l_pos_r_pos.clone() * res_is_gt.expr(meta)
                            - l_neg_r_neg.clone() * res_is_lt.expr(meta)
                            - res_is_eq.expr(meta)),
                ]
            }),
        );

        Box::new(RelConfig {
            diff_inv,
            res_is_eq,
            res_is_lt,
            res_is_gt,
            lhs,
            rhs,
            diff,
            lookup_stack_read_lhs,
            lookup_stack_read_rhs,
            lookup_stack_write_res,
            res,
            op_is_eq,
            op_is_ne,
            op_is_lt,
            op_is_gt,
            op_is_le,
            op_is_ge,
            op_is_sign,
            is_four_bytes,
            is_eight_bytes,
            is_sign,
            lhs_leading_bit,
            rhs_leading_bit,
            lhs_rem_value,
            rhs_rem_value,
        })
    }
}

impl<F: FieldExt> EventTableOpcodeConfig<F> for RelConfig {
    fn opcode(&self, meta: &mut VirtualCells<'_, F>) -> Expression<F> {
        let vtype = self.is_four_bytes.expr(meta) * constant_from!(4)
            + self.is_eight_bytes.expr(meta) * constant_from!(6)
            + self.is_sign.expr(meta)
            + constant_from!(1);

        let subop_lt_u = |meta: &mut VirtualCells<F>| {
            self.op_is_lt.expr(meta)
                * (constant_from!(1) - self.op_is_sign.expr(meta))
                * constant!(bn_to_field(
                    &(BigUint::from(RelOp::UnsignedLt as u64) << OPCODE_ARG0_SHIFT)
                ))
        };
        let subop_le_u = |meta: &mut VirtualCells<F>| {
            self.op_is_le.expr(meta)
                * (constant_from!(1) - self.op_is_sign.expr(meta))
                * constant!(bn_to_field(
                    &(BigUint::from(RelOp::UnsignedLe as u64) << OPCODE_ARG0_SHIFT)
                ))
        };

        let subop = |meta: &mut VirtualCells<F>| subop_le_u(meta) + subop_lt_u(meta);

        constant!(bn_to_field(
            &(BigUint::from(OpcodeClass::Rel as u64) << OPCODE_CLASS_SHIFT)
        )) + subop(meta)
            + vtype * constant!(bn_to_field(&(BigUint::from(1u64) << OPCODE_ARG1_SHIFT)))
    }

    fn assign(
        &self,
        ctx: &mut Context<'_, F>,
        step_info: &StepStatus,
        entry: &EventTableEntry,
    ) -> Result<(), Error> {
        let (class, vtype, lhs, rhs, value, diff) = match entry.step_info {
            StepInfo::I32Comp {
                class,
                left,
                right,
                value,
            } => {
                let vtype = VarType::I32;
                let lhs = left as u32 as u64;
                let rhs = right as u32 as u64;
                let diff = if lhs < rhs { rhs - lhs } else { lhs - rhs };

                (class, vtype, lhs, rhs, value, diff)
            }

            _ => unreachable!(),
        };

        self.is_four_bytes.assign(ctx, true)?;
        self.is_sign.assign(ctx, true)?;

        self.lhs.assign(ctx, lhs)?;
        self.rhs.assign(ctx, rhs)?;
        self.diff.assign(ctx, diff)?;

        self.diff_inv
            .assign(ctx, F::from(diff).invert().unwrap_or(F::zero()))?;
        self.res_is_eq.assign(ctx, lhs == rhs)?;
        self.res_is_gt.assign(ctx, lhs > rhs)?;
        self.res_is_lt.assign(ctx, lhs < rhs)?;
        self.res
            .assign(ctx, if value { F::one() } else { F::zero() })?;

        match class {
            RelOp::Eq => todo!(),
            RelOp::Ne => todo!(),
            RelOp::SignedGt => todo!(),
            RelOp::UnsignedGt => todo!(),
            RelOp::SignedGe => todo!(),
            RelOp::UnsignedGe => todo!(),
            RelOp::UnsignedLt => {
                self.op_is_lt.assign(ctx, true)?;
            }
            RelOp::UnsignedLe => {
                self.op_is_le.assign(ctx, true)?;
            }
        };

        self.lookup_stack_read_lhs.assign(
            ctx,
            &MemoryTableConfig::<F>::encode_stack_read(
                BigUint::from(step_info.current.eid),
                BigUint::from(1 as u64),
                BigUint::from(step_info.current.sp + 1),
                BigUint::from(vtype as u16),
                BigUint::from(rhs),
            ),
        )?;

        self.lookup_stack_read_rhs.assign(
            ctx,
            &MemoryTableConfig::<F>::encode_stack_read(
                BigUint::from(step_info.current.eid),
                BigUint::from(2 as u64),
                BigUint::from(step_info.current.sp + 2),
                BigUint::from(vtype as u16),
                BigUint::from(lhs),
            ),
        )?;

        self.lookup_stack_write_res.assign(
            ctx,
            &MemoryTableConfig::<F>::encode_stack_write(
                BigUint::from(step_info.current.eid),
                BigUint::from(3 as u64),
                BigUint::from(step_info.current.sp + 2),
                BigUint::from(VarType::I32 as u64),
                BigUint::from(value as u64),
            ),
        )?;

        Ok(())
    }

    fn opcode_class(&self) -> OpcodeClass {
        OpcodeClass::Rel
    }

    fn mops(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant_from!(3))
    }

    fn mtable_lookup(
        &self,
        meta: &mut VirtualCells<'_, F>,
        item: MLookupItem,
        common_config: &EventTableCommonConfig<F>,
    ) -> Option<Expression<F>> {
        let vtype = self.is_four_bytes.expr(meta) * constant_from!(4)
            + self.is_eight_bytes.expr(meta) * constant_from!(6)
            + self.is_sign.expr(meta)
            + constant_from!(1);

        match item {
            MLookupItem::First => Some(MemoryTableConfig::<F>::encode_stack_read(
                common_config.eid(meta),
                constant_from!(1),
                common_config.sp(meta) + constant_from!(1),
                vtype.clone(),
                self.rhs.expr(meta),
            )),
            MLookupItem::Second => Some(MemoryTableConfig::<F>::encode_stack_read(
                common_config.eid(meta),
                constant_from!(2),
                common_config.sp(meta) + constant_from!(2),
                vtype.clone(),
                self.lhs.expr(meta),
            )),
            MLookupItem::Third => Some(MemoryTableConfig::<F>::encode_stack_write(
                common_config.eid(meta),
                constant_from!(3),
                common_config.sp(meta) + constant_from!(2),
                constant_from!(VarType::I32),
                self.res.expr(meta),
            )),
            MLookupItem::Fourth => None,
        }
    }

    fn sp_diff(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant!(F::one()))
    }
}

#[cfg(test)]
mod tests {
    use crate::test::test_circuit_builder::test_circuit_noexternal;

    #[test]
    fn test_i32_le_u_1_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 1)
                      (i32.const 0)
                      (i32.le_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }

    #[test]
    fn test_i32_le_u_2_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 1)
                      (i32.const 1)
                      (i32.le_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }

    #[test]
    fn test_i32_le_u_3_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 0)
                      (i32.const 1)
                      (i32.le_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }

    #[test]
    fn test_i32_lt_u_1_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 1)
                      (i32.const 0)
                      (i32.lt_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }

    #[test]
    fn test_i32_lt_u_2_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 1)
                      (i32.const 1)
                      (i32.lt_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }

    #[test]
    fn test_i32_lt_u_3_ok() {
        let textual_repr = r#"
                (module
                    (func (export "test")
                      (i32.const 0)
                      (i32.const 1)
                      (i32.lt_u)
                      (drop)
                    )
                   )
                "#;

        test_circuit_noexternal(textual_repr).unwrap()
    }
}