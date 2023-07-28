use lambdaworks_crypto::fiat_shamir::default_transcript::DefaultTranscript;

use crate::{
    cairo::{
        air::{
            generate_cairo_proof, verify_cairo_proof, CairoAIR, MemorySegment, MemorySegmentMap,
            PublicInputs, FRAME_DST_ADDR, FRAME_OP0_ADDR, FRAME_OP1_ADDR, FRAME_PC,
        },
        cairo_layout::CairoLayout,
        execution_trace::build_main_trace,
        runner::run::{generate_prover_args, run_program, CairoVersion},
        tests::utils::{
            cairo0_program_path, cairo1_program_path, test_prove_cairo1_program,
            test_prove_cairo_program,
        },
    },
    starks::{
        debug::validate_trace,
        domain::Domain,
        proof::options::{ProofOptions, SecurityLevel},
        trace::TraceTable,
        traits::AIR,
    },
    FE,
};

#[test_log::test]
fn test_prove_cairo_simple_program() {
    let layout = CairoLayout::Plain;
    test_prove_cairo_program(&cairo0_program_path("simple_program.json"), &None, layout);
}

#[test_log::test]
fn test_prove_cairo_fibonacci_5() {
    let layout = CairoLayout::Plain;
    test_prove_cairo_program(&cairo0_program_path("fibonacci_5.json"), &None, layout);
}

// #[test_log::test]
// fn test_prove_cairo_fibonacci_5_proof_mode() {
//     test_prove_cairo_program(
//         &cairo0_program_path("fibonacci_5_proof_mode.json"),
//         &None,
//         true,
//     );
// }

// #[cfg_attr(feature = "metal", ignore)]
// #[test_log::test]
// fn test_prove_cairo_fibonacci_casm() {
//     let layout = CairoLayout::Plain;
//     test_prove_cairo1_program(&cairo1_program_path("fibonacci_cairo1.casm"), layout);
// }

#[test_log::test]
fn test_prove_cairo_rc_program() {
    let layout = CairoLayout::Small;
    test_prove_cairo_program(&cairo0_program_path("rc_program.json"), &None, layout);
}

#[test_log::test]
fn test_prove_cairo_lt_comparison() {
    let layout = CairoLayout::Small;
    test_prove_cairo_program(&cairo0_program_path("lt_comparison.json"), &None, layout);
}

// #[test_log::test]
// fn test_prove_cairo_lt_comparison_proof_mode() {
//     test_prove_cairo_program(
//         &cairo0_program_path("lt_comparison_proof_mode.json"),
//         &None,
//         true,
//     );
// }

#[cfg_attr(feature = "metal", ignore)]
#[test_log::test]
fn test_prove_cairo_compare_lesser_array() {
    let layout = CairoLayout::Small;
    test_prove_cairo_program(
        &cairo0_program_path("compare_lesser_array.json"),
        &None,
        layout,
    );
}

// #[cfg_attr(feature = "metal", ignore)]
// #[test_log::test]
// fn test_prove_cairo_compare_lesser_array_proof_mode() {
//     test_prove_cairo_program(
//         &cairo0_program_path("compare_lesser_array_proof_mode.json"),
//         &None,
//         true,
//     );
// }

#[test_log::test]
fn test_prove_cairo_output_and_rc_program() {
    let layout = CairoLayout::Small;
    test_prove_cairo_program(
        &cairo0_program_path("signed_div_rem.json"),
        &Some(289..293),
        layout,
    );
}

// #[test_log::test]
// fn test_prove_cairo_output_and_rc_program_proof_mode() {
//     test_prove_cairo_program(
//         &cairo0_program_path("signed_div_rem_proof_mode.json"),
//         &Some(297..301),
//         true,
//     );
// }

#[test_log::test]
fn test_verifier_rejects_proof_of_a_slightly_different_program() {
    let program_content = std::fs::read(cairo0_program_path("simple_program.json")).unwrap();
    let (main_trace, mut pub_input) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &None,
        CairoLayout::Plain,
    )
    .unwrap();

    let proof_options = ProofOptions::default_test_options();

    let proof = generate_cairo_proof(&main_trace, &pub_input, &proof_options).unwrap();

    // We modify the original program and verify using this new "corrupted" version
    let mut corrupted_program = pub_input.public_memory.clone();
    corrupted_program.insert(FE::one(), FE::from(5));
    corrupted_program.insert(FE::from(3), FE::from(5));

    // Here we use the corrupted version of the program in the public inputs
    pub_input.public_memory = corrupted_program;
    assert!(!verify_cairo_proof(&proof, &pub_input, &proof_options));
}

#[test_log::test]
fn test_verifier_rejects_proof_with_different_range_bounds() {
    let program_content = std::fs::read(cairo0_program_path("simple_program.json")).unwrap();
    let (main_trace, mut pub_inputs) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &None,
        CairoLayout::Plain,
    )
    .unwrap();

    let proof_options = ProofOptions::default_test_options();
    let proof = generate_cairo_proof(&main_trace, &pub_inputs, &proof_options).unwrap();

    pub_inputs.range_check_min = Some(pub_inputs.range_check_min.unwrap() + 1);
    assert!(!verify_cairo_proof(&proof, &pub_inputs, &proof_options));

    pub_inputs.range_check_min = Some(pub_inputs.range_check_min.unwrap() - 1);
    pub_inputs.range_check_max = Some(pub_inputs.range_check_max.unwrap() - 1);
    assert!(!verify_cairo_proof(&proof, &pub_inputs, &proof_options));
}

#[test_log::test]
fn test_verifier_rejects_proof_with_changed_range_check_value() {
    // In this test we change the range-check value in the trace, so the constraint
    // that asserts that the sum of the rc decomposed values is equal to the
    // range-checked value won't hold, and the verifier will reject the proof.
    let program_content = std::fs::read(cairo0_program_path("rc_program.json")).unwrap();
    let (main_trace, pub_inputs) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &None,
        CairoLayout::Small,
    )
    .unwrap();

    // The malicious value, we change the previous value to a 35.
    let malicious_rc_value = FE::from(35);

    let proof_options = ProofOptions::default_test_options();

    let mut malicious_trace_columns = main_trace.cols();
    let n_cols = malicious_trace_columns.len();
    let mut last_column = malicious_trace_columns.last().unwrap().clone();
    last_column[0] = malicious_rc_value;
    malicious_trace_columns[n_cols - 1] = last_column;

    let malicious_trace = TraceTable::new_from_cols(&malicious_trace_columns);
    let proof = generate_cairo_proof(&malicious_trace, &pub_inputs, &proof_options).unwrap();
    assert!(!verify_cairo_proof(&proof, &pub_inputs, &proof_options));
}

#[test_log::test]
fn test_verifier_rejects_proof_with_overflowing_range_check_value() {
    // In this test we manually insert a value greater than 2^128 in the range-check builtin segment.

    // This value is greater than 2^128, and the verifier should reject the proof built with it.
    let overflowing_rc_value = FE::from_hex("0x100000000000000000000000000000001").unwrap();
    let program_content = std::fs::read(cairo0_program_path("rc_program.json")).unwrap();
    let (register_states, mut malicious_memory, program_size, _) = run_program(
        None,
        CairoLayout::Small,
        &program_content,
        &CairoVersion::V0,
        false,
    )
    .unwrap();

    // The malicious value is inserted in memory here.
    malicious_memory.data.insert(27, overflowing_rc_value);

    // These is the regular setup for generating the trace and the Cairo AIR, but now
    // we do it with the malicious memory
    let proof_options = ProofOptions::default_test_options();
    let memory_segments = MemorySegmentMap::from([(MemorySegment::RangeCheck, 27..29)]);

    let mut pub_inputs = PublicInputs::from_regs_and_mem(
        &register_states,
        &malicious_memory,
        program_size,
        &memory_segments,
    );

    let malicious_trace = build_main_trace(&register_states, &malicious_memory, &mut pub_inputs);

    let proof = generate_cairo_proof(&malicious_trace, &pub_inputs, &proof_options).unwrap();
    assert!(!verify_cairo_proof(&proof, &pub_inputs, &proof_options));
}

#[test_log::test]
fn test_verifier_rejects_proof_with_changed_output() {
    let program_content = std::fs::read(cairo0_program_path("output_program.json")).unwrap();
    let (main_trace, pub_inputs) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &Some(27..28),
        CairoLayout::Small,
    )
    .unwrap();

    // The malicious value, we change the previous value to a 100.
    let malicious_output_value = FE::from(100);

    let mut output_col_idx = None;
    let mut output_row_idx = None;
    for (i, row) in main_trace.rows().iter().enumerate() {
        let output_col_found = [FRAME_PC, FRAME_DST_ADDR, FRAME_OP0_ADDR, FRAME_OP1_ADDR]
            .iter()
            .find(|&&col_idx| row[col_idx] == FE::from(27));
        if output_col_found.is_some() {
            output_col_idx = output_col_found;
            output_row_idx = Some(i);
            break;
        }
    }

    let output_col_idx = *output_col_idx.unwrap();
    let output_row_idx = output_row_idx.unwrap();

    let proof_options = ProofOptions::default_test_options();

    let mut malicious_trace_columns = main_trace.cols();
    let mut output_value_column = malicious_trace_columns[output_col_idx + 4].clone();
    output_value_column[output_row_idx] = malicious_output_value;
    malicious_trace_columns[output_col_idx + 4] = output_value_column;

    let malicious_trace = TraceTable::new_from_cols(&malicious_trace_columns);
    let proof = generate_cairo_proof(&malicious_trace, &pub_inputs, &proof_options).unwrap();
    assert!(!verify_cairo_proof(&proof, &pub_inputs, &proof_options));
}

#[test_log::test]
fn test_verifier_rejects_proof_with_different_security_params() {
    let program_content = std::fs::read(cairo0_program_path("output_program.json")).unwrap();
    let (main_trace, pub_inputs) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &None,
        CairoLayout::Small,
    )
    .unwrap();

    let proof_options_prover = ProofOptions::new_secure(SecurityLevel::Conjecturable80Bits, 3);

    let proof = generate_cairo_proof(&main_trace, &pub_inputs, &proof_options_prover).unwrap();

    let proof_options_verifier = ProofOptions::new_secure(SecurityLevel::Conjecturable128Bits, 3);

    assert!(!verify_cairo_proof(
        &proof,
        &pub_inputs,
        &proof_options_verifier
    ));
}

#[test]
fn check_simple_cairo_trace_evaluates_to_zero() {
    let program_content = std::fs::read(cairo0_program_path("simple_program.json")).unwrap();
    let (main_trace, public_input) = generate_prover_args(
        &program_content,
        &CairoVersion::V0,
        &None,
        CairoLayout::Plain,
    )
    .unwrap();
    let mut trace_polys = main_trace.compute_trace_polys();
    let mut transcript = DefaultTranscript::new();

    let proof_options = ProofOptions::default_test_options();
    let cairo_air = CairoAIR::new(main_trace.n_rows(), &public_input, &proof_options);
    let rap_challenges = cairo_air.build_rap_challenges(&mut transcript);

    let aux_trace = cairo_air.build_auxiliary_trace(&main_trace, &rap_challenges);
    let aux_polys = aux_trace.compute_trace_polys();

    trace_polys.extend_from_slice(&aux_polys);

    let domain = Domain::new(&cairo_air);

    assert!(validate_trace(
        &cairo_air,
        &trace_polys,
        &domain,
        &rap_challenges
    ));
}
