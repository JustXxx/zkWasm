use delphinus_zkwasm::runtime::host::ForeignContext;
use delphinus_zkwasm::runtime::host::ForeignStatics;
use std::rc::Rc;
use wasmi::tracer::Observer;

use crate::HostEnv;
use zkwasm_host_circuits::host::ForeignInst::WitnessInsert;
use zkwasm_host_circuits::host::ForeignInst::WitnessPop;
use zkwasm_host_circuits::host::ForeignInst::WitnessTraceSize;

#[derive(Default)]
pub struct WitnessContext {
    pub buf: Vec<u64>,
}

impl WitnessContext {
    pub fn witness_insert(&mut self, new: u64) {
        self.buf.insert(0, new);
    }

    pub fn witness_pop(&mut self) -> u64 {
        self.buf.pop().unwrap()
    }
}

impl ForeignContext for WitnessContext {
    fn get_statics(&self) -> Option<ForeignStatics> {
        None
    }
}

use specs::external_host_call_table::ExternalHostCallSignature;
pub fn register_witness_foreign(env: &mut HostEnv) {
    let foreign_witness_plugin = env
        .external_env
        .register_plugin("foreign_witness", Box::new(WitnessContext::default()));

    env.external_env.register_function(
        "wasm_witness_insert",
        WitnessInsert as usize,
        ExternalHostCallSignature::Argument,
        foreign_witness_plugin.clone(),
        Rc::new(
            |_obs: &Observer, context: &mut dyn ForeignContext, args: wasmi::RuntimeArgs| {
                let context = context.downcast_mut::<WitnessContext>().unwrap();
                context.witness_insert(args.nth::<u64>(0) as u64);
                None
            },
        ),
    );

    env.external_env.register_function(
        "wasm_witness_pop",
        WitnessPop as usize,
        ExternalHostCallSignature::Return,
        foreign_witness_plugin.clone(),
        Rc::new(
            |_obs: &Observer, context: &mut dyn ForeignContext, _args: wasmi::RuntimeArgs| {
                let context = context.downcast_mut::<WitnessContext>().unwrap();
                Some(wasmi::RuntimeValue::I64(context.witness_pop() as i64))
            },
        ),
    );

    env.external_env.register_function(
        "wasm_trace_size",
        WitnessTraceSize as usize,
        ExternalHostCallSignature::Return,
        foreign_witness_plugin.clone(),
        Rc::new(
            |obs: &Observer, _context: &mut dyn ForeignContext, _args: wasmi::RuntimeArgs| {
                Some(wasmi::RuntimeValue::I64(obs.counter as i64))
            },
        ),
    );
}