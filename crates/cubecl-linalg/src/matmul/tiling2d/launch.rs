use std::cmp::max;

use cubecl_core::{prelude::*, Compiler};

use crate::{
    matmul::tiling2d::{
        base::tiling2d_cube_kernel,
        config::{tiling2d_cube_count, tiling2d_cube_dim, CubeTiling2dConfig},
    },
    tensor::{matrix_layout, MatrixLayout, TensorHandle},
};

use super::config::Tiling2dConfig;

/// Matrix multiplication using tiling 2d algorithm.
pub fn matmul_tiling_2d<R: Runtime, F: Float>(
    client: &ComputeClient<R::Server, R::Channel>,
    lhs: TensorHandle<R, F>,
    rhs: TensorHandle<R, F>,
    out: TensorHandle<R, F>,
    config: Tiling2dConfig,
) -> TensorHandle<R, F> {
    matmul_tiling_2d_ref::<R, F>(client, lhs.as_ref(), rhs.as_ref(), out.as_ref(), config);

    out
}

/// Matrix multiplication using tiling 2d algorithm.
pub fn matmul_tiling_2d_ref<R: Runtime, F: Float>(
    client: &ComputeClient<R::Server, R::Channel>,
    lhs: TensorHandleRef<'_, R>,
    rhs: TensorHandleRef<'_, R>,
    out: TensorHandleRef<'_, R>,
    config: Tiling2dConfig,
) {
    assert!(
        config.block_size_k * max(config.block_size_m, config.block_size_n)
            <= <R::Compiler as Compiler>::max_shared_memory_size(),
        "Shared memory limit will be busted. "
    );
    let rank = lhs.strides.len();

    let m = lhs.shape[rank - 2];
    let k = lhs.shape[rank - 1];
    let n = rhs.shape[rank - 1];

    let check_layout = |strides: &[usize]| match matrix_layout(strides) {
        MatrixLayout::Contiguous => false,
        MatrixLayout::MildlyPermuted {
            transposed,
            batch_swap: _,
        } => transposed,
        MatrixLayout::HighlyPermuted => {
            panic!("Can't run on highly permuted tensor")
        }
    };
    let lhs_transposed = check_layout(lhs.strides);
    let rhs_transposed = check_layout(rhs.strides);

    let vectorization = |shape: usize| {
        [].into_iter()
            .filter(|v| shape % v == 0)
            .map(|v| v as u8)
            .next()
            .unwrap_or(1)
    };

    let lhs_vectorization = match lhs_transposed {
        true => vectorization(m),
        false => 1,
    };
    let rhs_vectorization = match rhs_transposed {
        true => 1,
        false => vectorization(n),
    };
    let out_vectorization = vectorization(n);

    let cube_count = tiling2d_cube_count::<R>(out.shape, &config);
    let cube_dim = tiling2d_cube_dim(&config);
    let cube_config = CubeTiling2dConfig::new(&config, m, k, n, lhs_transposed, rhs_transposed);

    tiling2d_cube_kernel::launch::<F, R>(
        client,
        cube_count,
        cube_dim,
        TensorArg::vectorized(lhs_vectorization, lhs.handle, lhs.strides, lhs.shape),
        TensorArg::vectorized(rhs_vectorization, rhs.handle, rhs.strides, rhs.shape),
        TensorArg::vectorized(out_vectorization, out.handle, out.strides, out.shape),
        cube_config,
    );
}
