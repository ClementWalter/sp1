//! A duplex challenger for Poseidon2 over BN254.

use p3_field::AbstractField;
use p3_field::Field;
use sp1_recursion_compiler::ir::Ext;
use sp1_recursion_compiler::ir::{Builder, Config, Felt, Var};

use crate::poseidon2::P2CircuitBuilder;
use crate::DIGEST_SIZE;

const WIDTH: usize = 3;

pub struct MultiFieldChallengerVariable<C: Config> {
    sponge_state: [Var<C::N>; 3],
    input_buffer: Vec<Felt<C::F>>,
    output_buffer: Vec<Felt<C::F>>,
    num_f_elms: usize,
}

pub fn reduce_32<C: Config>(builder: &mut Builder<C>, vals: &[Felt<C::F>]) -> Var<C::N> {
    let mut power = C::N::one();
    let result: Var<C::N> = builder.eval(C::N::zero());
    for val in vals.iter() {
        let bits = builder.num2bits_f_circuit(*val);
        let val = builder.bits_to_num_var_circuit(&bits);
        builder.assign(result, result + val * power);
        power *= C::N::from_canonical_usize(1usize << 32);
    }
    result
}

pub fn split_32<C: Config>(builder: &mut Builder<C>, val: Var<C::N>, n: usize) -> Vec<Felt<C::F>> {
    let bits = builder.num2bits_v_circuit(val, 256);
    let mut results = Vec::new();
    for i in 0..n {
        let result: Felt<C::F> = builder.eval(C::F::zero());
        for j in 0..32 {
            let bit = bits[i * 32 + j];
            let t = builder.eval(result + C::F::from_wrapped_u32(1 << j));
            let z = builder.select_f(bit, t, result);
            builder.assign(result, z);
        }
        results.push(result);
    }
    results
}

impl<C: Config> MultiFieldChallengerVariable<C> {
    pub fn new(builder: &mut Builder<C>) -> Self {
        MultiFieldChallengerVariable::<C> {
            sponge_state: [
                builder.eval(C::N::zero()),
                builder.eval(C::N::zero()),
                builder.eval(C::N::zero()),
            ],
            input_buffer: vec![],
            output_buffer: vec![],
            num_f_elms: C::N::bits() / C::F::bits(),
        }
    }

    pub fn duplexing(&mut self, builder: &mut Builder<C>) {
        assert!(self.input_buffer.len() <= self.num_f_elms * WIDTH);

        for (i, f_chunk) in self.input_buffer.chunks(self.num_f_elms).enumerate() {
            self.sponge_state[i] = reduce_32(builder, f_chunk);
        }
        self.input_buffer.clear();

        builder.p2_permute_mut(self.sponge_state);

        self.output_buffer.clear();
        for &pf_val in self.sponge_state.iter() {
            let f_vals = split_32(builder, pf_val, self.num_f_elms);
            for f_val in f_vals {
                self.output_buffer.push(f_val);
            }
        }
    }

    pub fn observe(&mut self, builder: &mut Builder<C>, value: Felt<C::F>) {
        self.output_buffer.clear();

        self.input_buffer.push(value);
        if self.input_buffer.len() == self.num_f_elms * WIDTH {
            self.duplexing(builder);
        }
    }

    pub fn observe_commitment(
        &mut self,
        builder: &mut Builder<C>,
        value: [Var<C::N>; DIGEST_SIZE],
    ) {
        for i in 0..DIGEST_SIZE {
            let f_vals: Vec<Felt<C::F>> = split_32(builder, value[i], self.num_f_elms);
            for f_val in f_vals {
                self.observe(builder, f_val);
            }
        }
    }

    pub fn sample(&mut self, builder: &mut Builder<C>) -> Felt<C::F> {
        if !self.input_buffer.is_empty() || self.output_buffer.is_empty() {
            self.duplexing(builder);
        }

        self.output_buffer
            .pop()
            .expect("output buffer should be non-empty")
    }

    pub fn sample_ext(&mut self, builder: &mut Builder<C>) -> Ext<C::F, C::EF> {
        let a = self.sample(builder);
        let b = self.sample(builder);
        let c = self.sample(builder);
        let d = self.sample(builder);
        builder.ext_from_base_slice(&[a, b, c, d])
    }

    pub fn sample_bits(&mut self, builder: &mut Builder<C>, bits: usize) -> Var<C::N> {
        let rand_f = self.sample(builder);
        let rand_f_bits = builder.num2bits_f_circuit(rand_f);
        builder.bits_to_num_var_circuit(&rand_f_bits[0..bits])
    }

    pub fn check_witness(&mut self, builder: &mut Builder<C>, bits: usize, witness: Felt<C::F>) {
        self.observe(builder, witness);
        let element = self.sample_bits(builder, bits);
        builder.assert_var_eq(element, C::N::from_canonical_usize(0));
    }
}

#[cfg(test)]
mod tests {
    use p3_baby_bear::BabyBear;
    use p3_bn254_fr::{Bn254Fr, DiffusionMatrixBN254};
    use p3_challenger::FieldChallenger;
    use p3_challenger::{CanObserve, CanSample};
    use p3_field::extension::BinomialExtensionField;
    use p3_field::reduce_32 as reduce_32_gt;
    use p3_field::split_32 as split_32_gt;
    use p3_field::AbstractField;
    use p3_symmetric::Hash;
    use sp1_recursion_compiler::ir::Builder;
    use sp1_recursion_compiler::ir::SymbolicExt;
    use sp1_recursion_compiler::r1cs::{gnark, R1CSBackend};
    use sp1_recursion_compiler::OuterConfig;
    use sp1_recursion_core::stark::config::{OuterChallenger, OuterPerm};

    use super::reduce_32;
    use super::split_32;
    use crate::DIGEST_SIZE;
    use crate::{challenger::MultiFieldChallengerVariable, poseidon2::tests::bn254_poseidon2_rc3};

    #[test]
    fn test_num2bits_v() {
        let mut builder = Builder::<OuterConfig>::default();
        let mut value_u32 = 1345237507;
        let value = builder.eval(Bn254Fr::from_canonical_u32(value_u32));
        let result = builder.num2bits_v_circuit(value, 32);
        for i in 0..result.len() {
            builder.assert_var_eq(result[i], Bn254Fr::from_canonical_u32(value_u32 & 1));
            value_u32 >>= 1;
        }

        let mut backend = R1CSBackend::<OuterConfig>::default();
        let constraints = backend.emit(builder.operations);
        gnark::ffi_test_circuit(constraints);
    }

    #[test]
    fn test_reduce_32() {
        let value_1 = BabyBear::from_canonical_u32(1345237507);
        let value_2 = BabyBear::from_canonical_u32(1000001);
        let gt: Bn254Fr = reduce_32_gt(&[value_1, value_2]);

        let mut builder = Builder::<OuterConfig>::default();
        let value_1 = builder.eval(value_1);
        let value_2 = builder.eval(value_2);
        let result = reduce_32(&mut builder, &[value_1, value_2]);
        builder.assert_var_eq(result, gt);

        let mut backend = R1CSBackend::<OuterConfig>::default();
        let constraints = backend.emit(builder.operations);
        gnark::ffi_test_circuit(constraints);
    }

    #[test]
    fn test_split_32() {
        let value = Bn254Fr::from_canonical_u32(1345237507);
        let gt: Vec<BabyBear> = split_32_gt(value, 3);

        let mut builder = Builder::<OuterConfig>::default();
        let value = builder.eval(value);
        let result = split_32(&mut builder, value, 3);

        builder.assert_felt_eq(result[0], gt[0]);
        builder.assert_felt_eq(result[1], gt[1]);
        builder.assert_felt_eq(result[2], gt[2]);

        let mut backend = R1CSBackend::<OuterConfig>::default();
        let constraints = backend.emit(builder.operations);
        gnark::ffi_test_circuit(constraints);
    }

    #[test]
    fn test_challenger() {
        let perm = OuterPerm::new(8, 56, bn254_poseidon2_rc3(), DiffusionMatrixBN254);
        let mut challenger = OuterChallenger::new(perm).unwrap();
        let a = BabyBear::from_canonical_usize(1);
        let b = BabyBear::from_canonical_usize(2);
        let c = BabyBear::from_canonical_usize(3);
        challenger.observe(a);
        challenger.observe(b);
        challenger.observe(c);
        let gt1: BabyBear = challenger.sample();
        challenger.observe(a);
        challenger.observe(b);
        challenger.observe(c);
        let gt2: BabyBear = challenger.sample();

        let mut builder = Builder::<OuterConfig>::default();
        let mut challenger = MultiFieldChallengerVariable::new(&mut builder);
        let a = builder.eval(a);
        let b = builder.eval(b);
        let c = builder.eval(c);
        challenger.observe(&mut builder, a);
        challenger.observe(&mut builder, b);
        challenger.observe(&mut builder, c);
        let result1 = challenger.sample(&mut builder);
        builder.assert_felt_eq(gt1, result1);
        challenger.observe(&mut builder, a);
        challenger.observe(&mut builder, b);
        challenger.observe(&mut builder, c);
        let result2 = challenger.sample(&mut builder);
        builder.assert_felt_eq(gt2, result2);

        let mut backend = R1CSBackend::<OuterConfig>::default();
        let constraints = backend.emit(builder.operations);
        gnark::ffi_test_circuit(constraints);
    }

    #[test]
    fn test_challenger_sample_ext() {
        let perm = OuterPerm::new(8, 56, bn254_poseidon2_rc3(), DiffusionMatrixBN254);
        let mut challenger = OuterChallenger::new(perm).unwrap();
        let a = BabyBear::from_canonical_usize(1);
        let b = BabyBear::from_canonical_usize(2);
        let c = BabyBear::from_canonical_usize(3);
        let hash = Hash::from([Bn254Fr::two(); DIGEST_SIZE]);
        challenger.observe(hash);
        challenger.observe(a);
        challenger.observe(b);
        challenger.observe(c);
        let gt1: BinomialExtensionField<BabyBear, 4> = challenger.sample_ext_element();
        challenger.observe(a);
        challenger.observe(b);
        challenger.observe(c);
        let gt2: BinomialExtensionField<BabyBear, 4> = challenger.sample_ext_element();

        let mut builder = Builder::<OuterConfig>::default();
        let mut challenger = MultiFieldChallengerVariable::new(&mut builder);
        let a = builder.eval(a);
        let b = builder.eval(b);
        let c = builder.eval(c);
        let hash = builder.eval(Bn254Fr::two());
        challenger.observe_commitment(&mut builder, [hash]);
        challenger.observe(&mut builder, a);
        challenger.observe(&mut builder, b);
        challenger.observe(&mut builder, c);
        let result1 = challenger.sample_ext(&mut builder);
        challenger.observe(&mut builder, a);
        challenger.observe(&mut builder, b);
        challenger.observe(&mut builder, c);
        let result2 = challenger.sample_ext(&mut builder);

        builder.assert_ext_eq(SymbolicExt::Const(gt1), result1);
        builder.assert_ext_eq(SymbolicExt::Const(gt2), result2);

        let mut backend = R1CSBackend::<OuterConfig>::default();
        let constraints = backend.emit(builder.operations);
        gnark::ffi_test_circuit(constraints);
    }
}
