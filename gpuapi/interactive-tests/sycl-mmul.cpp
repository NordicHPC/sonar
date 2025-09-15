// XPU test case (maybe also Habana eventually).
//
// This was modified from
// <URL:https://github.com/intel/llvm-test-suite/blob/intel/SYCL/Matrix/joint_matrix_bfloat16.cpp>.
// See below for copyright notice.
//
// On eX3 XPU node eg n022 (note the 2025 version will not work with this code):
//
// $ module load intel/oneapi/2023.1/tbb
// $ module load intel/oneapi/2023.1/compiler-rt
// $ module load intel/oneapi/2023.1/compiler
// $ icpx -fsycl -DSYCL_EXT_ONEAPI_MATRIX_VERSION=4 sycl-mmul.cpp -o sycl-mmul
// $ ./sycl-mmul &
//
// It runs for about 45s on that node.
//
// While it's running, run `xpu-smi stats -d 0` or (from Sonar) `xpu-shell -state` or other similar
// commands to verify that the compute engine is busy, and run both to verify that they are in
// agreement.
//
// To schedule the load on specific devices, use an environment variable, here GPU 1:
//
// $ ONEAPI_DEVICE_SELECTOR='*:1' ./sycl-mmul
//
// There is a rich device selection language.  See
// <URL:https://github.com/intel/llvm/blob/sycl/sycl/doc/EnvironmentVariables.md#oneapi_device_selector>
// for further documentation.

//==-------- joint_matrix_bfloat16.cpp  - DPC++ joint_matrix----------- ----==//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
// REQUIRES: matrix

// RUN: %clangxx -fsycl %s -o %t.out -DSYCL_EXT_ONEAPI_MATRIX_VERSION=4
// RUN: %CPU_RUN_PLACEHOLDER %t.out
// RUN: %GPU_RUN_PLACEHOLDER %t.out

#include <iostream>
#include <ctime>
#include <sycl/sycl.hpp>

using namespace sycl;
using namespace sycl::ext::oneapi::experimental::matrix;
using bfloat16 = sycl::ext::oneapi::bfloat16;

#define SG_SZ 16

#define TM 8
#define TN SG_SZ
#define TK 16

#define BF16_EPSILON 0.00781250

template <typename T, size_t NUM_ROWS, size_t NUM_COLS> struct big_matrix {
private:
  T *mat;

public:
  T *get_data() { return mat; }
  void set_data(T *data) { mat = data; }
  big_matrix(T *data) : mat(data) {}
};

template <typename T1, typename T2, size_t M, size_t N, size_t K>
void matrix_multiply(big_matrix<T1, M, N> &C, big_matrix<T2, M, K> &A,
                     big_matrix<T2, K / 2, N * 2> &B) {
  size_t NDRangeM = M / TM;
  size_t NDRangeN = N / TN;
  buffer<bfloat16, 2> bufA(A.get_data(), range<2>(M, K));
  buffer<bfloat16, 2> bufB(B.get_data(), range<2>(K, N));
  buffer<float, 2> bufC((float *)C.get_data(), range<2>(M, N));

  queue q;
  q.submit([&](handler &cgh) {
     auto accC = bufC.get_access<access::mode::read_write>(cgh);
     auto accA = bufA.get_access<access::mode::read_write>(cgh);
     auto accB = bufB.get_access<access::mode::read_write>(cgh);

     cgh.parallel_for<class imatrix>(
         nd_range<2>({NDRangeM, NDRangeN * SG_SZ}, {1, 1 * SG_SZ}),
         [=](nd_item<2> spmd_item) [[intel::reqd_sub_group_size(SG_SZ)]]

         {
           // The submatrix API has to be accessed by all the workitems in a
           // subgroup these functions will be called once by the subgroup no
           // code divergence between the workitems
           const auto global_idx = spmd_item.get_global_id(0);
           const auto global_idy = spmd_item.get_global_id(1);
           const auto sg_startx = global_idx - spmd_item.get_local_id(0);
           const auto sg_starty = global_idy - spmd_item.get_local_id(1);

           sub_group sg = spmd_item.get_sub_group();
           joint_matrix<sub_group, bfloat16, use::a, TM, TK, layout::row_major>
               sub_a;
           // For B, we assume B has been already VNNIed.
           joint_matrix<sub_group, bfloat16, use::b, TK, TN,
                        ext::intel::experimental::matrix::layout::packed>
               sub_b;
           joint_matrix<sub_group, float, use::accumulator, TM, TN> sub_c;

           joint_matrix_load(sg, sub_c,
                             accC.get_pointer() + (sg_startx * TM) * N +
                                 sg_starty / SG_SZ * TN,
                             N, layout::row_major);
           for (int k = 0; k < K / TK; k += 1) { //
             joint_matrix_load(
                 sg, sub_a, accA.get_pointer() + (sg_startx * TM) * K + k * TK,
                 K);
             joint_matrix_load(sg, sub_b,
                               accB.get_pointer() + (k * TK / 2) * (N * 2) +
                                   sg_starty / SG_SZ * TN * 2,
                               N * 2);
             sub_c = joint_matrix_mad(sg, sub_a, sub_b, sub_c);
           }
           joint_matrix_store(sg, sub_c,
                              accC.get_pointer() + (sg_startx * TM) * N +
                                  sg_starty / SG_SZ * TN,
                              N, layout::row_major);
         }); // parallel for
   }).wait();
}

static constexpr size_t MATRIX_M = TM * 2000;
static constexpr size_t MATRIX_N = TN * 2000;
static constexpr size_t MATRIX_K = TK * 2000;

int main() {
  // bfloat16 A[MATRIX_M][MATRIX_K];
  // bfloat16 B[MATRIX_K / 2][MATRIX_N * 2];
  // float C[MATRIX_M][MATRIX_N];

  // Dynamic allocation or the linker will toss its cookies.
  bfloat16 (*A)[MATRIX_K] = (bfloat16 (*)[MATRIX_K])malloc(2*MATRIX_M*MATRIX_K);
  bfloat16 (*B)[MATRIX_N * 2] = (bfloat16(*)[MATRIX_N * 2])malloc(2*(MATRIX_K/2)*(MATRIX_N)*2);
  float (*C)[MATRIX_N] = (float(*)[MATRIX_N])malloc(4*MATRIX_M*MATRIX_N);

  for (int i = 0; i < MATRIX_M; i++) {
    for (int j = 0; j < MATRIX_K; j++) {
      A[i][j] = bfloat16(1.0f * (i + j));
    }
  }
  for (int i = 0; i < MATRIX_K / 2; i++) {
    for (int j = 0; j < MATRIX_N * 2; j++) {
      B[i][j] = bfloat16(2.0f * i + 3.0f * j);
    }
  }
  for (int i = 0; i < MATRIX_M; i++) {
    for (int j = 0; j < MATRIX_N; j++) {
      C[i][j] = 1.0;
    }
  }

  big_matrix<float, MATRIX_M, MATRIX_N> MC((float *)C);
  big_matrix<bfloat16, MATRIX_M, MATRIX_K> MA((bfloat16 *)A);
  big_matrix<bfloat16, MATRIX_K / 2, MATRIX_N * 2> MB((bfloat16 *)B);

  time_t then = time(NULL);
  matrix_multiply(MC, MA, MB);
  time_t now = time(NULL);
  printf("Running time: %lds\n", now-then);
}
