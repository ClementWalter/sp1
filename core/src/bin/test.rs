use std::sync::Arc;

use sp1_core::{
    air::MachineAir,
    runtime::{ExecutionRecord, Program, Runtime},
    stark::{LocalProver, MachineRecord, Prover, RiscvAir, StarkGenericConfig},
    utils::{prove, prove_simple, setup_logger, BabyBearPoseidon2, BabyBearPoseidon2Inner},
};

fn main() {
    setup_logger();
    let elf = include_bytes!("../../../tests/fibonacci/elf/riscv32im-succinct-zkvm-elf");
    let program = Program::from(elf);
    let shard_size = 1024;

    let config = BabyBearPoseidon2::new();
    let machine = RiscvAir::machine(config.clone());
    let chips = machine.chips();

    let mut runtime = Runtime::new(program.clone(), Default::default());
    runtime.shard_size = shard_size;
    runtime.shard_batch_size = 1;
    let mut shards = Vec::new();
    loop {
        let done = runtime.execute_shard_batch().unwrap();
        runtime.record.index = runtime.record.cpu_events.first().unwrap().shard;
        let mut shard = runtime.record.pop_shard(shard_size as usize);
        println!("index: {:?}", shard.index);
        chips.iter().for_each(|chip| {
            let mut output = ExecutionRecord::default();
            output.set_index(shard.index());
            chip.generate_dependencies(&shard, &mut output);
            shard.append(&mut output);
        });
        println!("index: {:?}", shard.cpu_events.first().unwrap().shard);
        // let sharded = shard.shard(&Default::default());
        // assert_eq!(sharded.len(), 1);
        // let shard = sharded.into_iter().next().unwrap();
        // shard_index += 1;
        // runtime.record.nonce_lookup.clone_from(&shard.nonce_lookup);
        runtime
            .record
            .nonce_lookup
            .extend(shard.nonce_lookup.clone());
        println!("shard stats: {:?}", shard.stats());
        shards.push(shard);
        if done {
            break;
        }
    }
    println!("remaining stats: {:?}", runtime.record.stats());
    let last_shard_pvs = shards.last().unwrap().public_values;
    for shard in shards.iter_mut() {
        shard.public_values.committed_value_digest = last_shard_pvs.committed_value_digest;
        shard.public_values.deferred_proofs_digest = last_shard_pvs.deferred_proofs_digest;
        shard.public_values.exit_code = last_shard_pvs.exit_code;

        shard.nonce_lookup = shard.get_lookup_ids();
        println!("num lookups: {:?}", shard.nonce_lookup.len());
    }
    // // runtime.run().unwrap();
    // println!("{:?}", runtime.record.stats());

    let (pk, vk) = machine.setup(&program);

    // // Get the local and global chips.
    // let mut record = runtime.record;
    // record.index = 1;
    // record.program = Arc::new(program.clone());
    // println!(
    //     "memory finalize events: {:?}",
    //     record.memory_finalize_events.len()
    // );

    // for cpu_event in record.cpu_events.iter() {
    //     // println!("cpu event: {:?}", cpu_event.shard);
    //     if cpu_event.shard != 1 {
    //         println!("cpu event: {:?}", cpu_event.shard);
    //     }
    // }

    // chips.iter().for_each(|chip| {
    //     let mut output = ExecutionRecord::default();
    //     output.set_index(record.index());
    //     chip.generate_dependencies(&record, &mut output);
    //     record.append(&mut output);
    // });

    // let shards = record.shard(&Default::default());

    let mut challenger = config.challenger();
    let proof =
        LocalProver::prove_shards(&machine, &pk, shards, &mut challenger, Default::default());

    let mut challenger = config.challenger();
    machine.verify(&vk, &proof, &mut challenger).unwrap();

    // let proof = prove_simple(config.clone(), runtime).unwrap();
    // let mut challenger = config.challenger();
    // machine.verify(&vk, &proof, &mut challenger).unwrap();
}
